pub mod error;
pub mod models;
pub mod encryption;
pub mod oauth;
pub mod drive;
pub mod protocol;

use crate::database::Database;
use drive::DriveClient;
use error::SyncError;
use models::{SyncSettings, SyncState, SyncStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

/// Global flag: is a sync currently running?
static IS_SYNCING: AtomicBool = AtomicBool::new(false);

/// Cached encryption key (derived from passphrase, held in memory for the session).
static ENCRYPTION_KEY: Lazy<Mutex<Option<[u8; 32]>>> = Lazy::new(|| Mutex::new(None));

/// Background sync abort handle
static SYNC_ABORT: Lazy<Mutex<Option<tokio::sync::watch::Sender<bool>>>> = Lazy::new(|| Mutex::new(None));

/// Get the current sync status from DB metadata.
pub async fn get_sync_status(db: &Database) -> SyncStatus {
    let last_sync_at = db.get_setting("sync_last_sync_at").await.unwrap_or(None);
    let connected_email = db.get_setting("sync_email").await.unwrap_or(None);
    let enabled = db.get_setting("sync_enabled").await.unwrap_or(None)
        .map(|v| v == "true").unwrap_or(false);

    let state = if !enabled {
        SyncState::Disabled
    } else if IS_SYNCING.load(Ordering::Relaxed) {
        SyncState::Syncing
    } else if connected_email.is_none() {
        SyncState::Disabled
    } else {
        SyncState::Idle
    };

    // Pending changes are counted against the push watermark (what hasn't been uploaded yet),
    // falling back to last_sync_at for installs upgrading from v1.8.6 or earlier.
    let push_base = db.get_setting("sync_push_base_at").await.unwrap_or(None)
        .or_else(|| last_sync_at.clone());
    let pending_changes = count_pending_changes(db, &push_base).await;

    SyncStatus {
        state,
        last_sync_at,
        pending_changes,
        error_message: None,
        connected_email,
    }
}

/// Count local changes since last sync.
async fn count_pending_changes(db: &Database, last_sync_at: &Option<String>) -> u64 {
    let since = match last_sync_at {
        Some(ts) => ts.clone(),
        None => "1970-01-01T00:00:00Z".to_string(),
    };

    let clip_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM clips WHERE updated_at > ?"
    ).bind(&since).fetch_one(&db.pool).await.unwrap_or(0);

    let folder_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM folders WHERE updated_at > ?"
    ).bind(&since).fetch_one(&db.pool).await.unwrap_or(0);

    let tombstone_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_tombstones WHERE deleted_at > ?"
    ).bind(&since).fetch_one(&db.pool).await.unwrap_or(0);

    (clip_count + folder_count + tombstone_count) as u64
}

/// Load sync settings from DB.
pub async fn get_sync_settings(db: &Database) -> SyncSettings {
    let enabled = db.get_setting("sync_enabled").await.unwrap_or(None)
        .map(|v| v == "true").unwrap_or(false);
    let interval = db.get_setting("sync_interval_seconds").await.unwrap_or(None)
        .and_then(|v| v.parse().ok()).unwrap_or(300u64);
    let sync_images = db.get_setting("sync_images").await.unwrap_or(None)
        .map(|v| v != "false").unwrap_or(true);

    SyncSettings { enabled, interval_seconds: interval, sync_images }
}

/// Save sync settings to DB.
pub async fn save_sync_settings(db: &Database, settings: &SyncSettings) -> Result<(), sqlx::Error> {
    save_setting(&db.pool, "sync_enabled", &settings.enabled.to_string()).await?;
    save_setting(&db.pool, "sync_interval_seconds", &settings.interval_seconds.to_string()).await?;
    save_setting(&db.pool, "sync_images", &settings.sync_images.to_string()).await?;
    Ok(())
}

/// Record a deletion tombstone for sync propagation.
pub async fn record_tombstone(db: &Database, uuid: &str, entity_type: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO sync_tombstones (uuid, entity_type, deleted_at) VALUES (?, ?, CURRENT_TIMESTAMP)"
    ).bind(uuid).bind(entity_type).execute(&db.pool).await?;
    Ok(())
}

/// Clean up tombstones older than 30 days.
pub async fn cleanup_tombstones(db: &Database) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM sync_tombstones WHERE deleted_at < datetime('now', '-30 days')"
    ).execute(&db.pool).await?;
    Ok(result.rows_affected())
}

/// Get the device_id for this installation.
pub async fn get_device_id(db: &Database) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM sync_meta WHERE key = 'device_id'")
        .fetch_optional(&db.pool).await.unwrap_or(None)
}

// ── Token management ──

/// Get a valid access token, refreshing if expired.
pub async fn get_valid_token(db: &Database) -> Result<String, SyncError> {
    let access_token = db.get_setting("sync_access_token").await
        .map_err(|e| SyncError::Database(e.to_string()))?
        .ok_or(SyncError::NotConfigured)?;
    let refresh_token = db.get_setting("sync_refresh_token").await
        .map_err(|e| SyncError::Database(e.to_string()))?
        .ok_or(SyncError::NotConfigured)?;
    let expires_at: i64 = db.get_setting("sync_token_expires_at").await
        .map_err(|e| SyncError::Database(e.to_string()))?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let now = chrono::Utc::now().timestamp();

    // Refresh if token expires within 5 minutes
    if now > expires_at - 300 {
        log::info!("SYNC: Refreshing expired access token");
        let new_tokens = oauth::refresh_token(&refresh_token).await?;
        save_setting(&db.pool, "sync_access_token", &new_tokens.access_token).await
            .map_err(|e| SyncError::Database(e.to_string()))?;
        save_setting(&db.pool, "sync_refresh_token", &new_tokens.refresh_token).await
            .map_err(|e| SyncError::Database(e.to_string()))?;
        save_setting(&db.pool, "sync_token_expires_at", &new_tokens.expires_at.to_string()).await
            .map_err(|e| SyncError::Database(e.to_string()))?;
        Ok(new_tokens.access_token)
    } else {
        Ok(access_token)
    }
}

/// Get the encryption key, deriving it from the cached passphrase.
pub fn get_encryption_key() -> Result<[u8; 32], SyncError> {
    ENCRYPTION_KEY.lock().ok_or(SyncError::Encryption("Sync passphrase not unlocked. Please enter your passphrase in Settings > Sync.".into()))
}

/// Set the encryption key from a passphrase.
pub async fn set_passphrase(db: &Database, passphrase: &str) -> Result<(), SyncError> {
    // Get or create salt
    let salt_hex = match db.get_setting("sync_encryption_salt").await
        .map_err(|e| SyncError::Database(e.to_string()))? {
        Some(s) => s,
        None => {
            let salt = encryption::generate_salt();
            let hex = salt.iter().map(|b| format!("{:02x}", b)).collect::<String>();
            save_setting(&db.pool, "sync_encryption_salt", &hex).await
                .map_err(|e| SyncError::Database(e.to_string()))?;
            hex
        }
    };

    let salt = decode_hex(&salt_hex)?;
    let key = encryption::derive_key(passphrase, &salt)?;

    // Store passphrase verification hash
    let hash = encryption::hash_passphrase_for_verification(passphrase)?;
    save_setting(&db.pool, "sync_passphrase_hash", &hash).await
        .map_err(|e| SyncError::Database(e.to_string()))?;

    // Cache key in memory
    *ENCRYPTION_KEY.lock() = Some(key);

    Ok(())
}

/// Verify a passphrase against the stored hash and cache the key.
pub async fn unlock_with_passphrase(db: &Database, passphrase: &str) -> Result<bool, SyncError> {
    let hash = db.get_setting("sync_passphrase_hash").await
        .map_err(|e| SyncError::Database(e.to_string()))?
        .ok_or(SyncError::Encryption("No passphrase has been set".into()))?;

    if !encryption::verify_passphrase(passphrase, &hash) {
        return Ok(false);
    }

    let salt_hex = db.get_setting("sync_encryption_salt").await
        .map_err(|e| SyncError::Database(e.to_string()))?
        .ok_or(SyncError::Encryption("Missing encryption salt".into()))?;

    let salt = decode_hex(&salt_hex)?;
    let key = encryption::derive_key(passphrase, &salt)?;
    *ENCRYPTION_KEY.lock() = Some(key);

    Ok(true)
}

// ── Main sync entry point (called by commands + background task) ──

/// Execute a sync cycle. Returns error message on failure.
pub async fn execute_sync(db: &Database) -> Result<String, String> {
    if IS_SYNCING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err("Sync already in progress".into());
    }

    let result = async {
        let token = get_valid_token(db).await.map_err(|e| e.to_string())?;
        let settings = get_sync_settings(db).await;
        let drive = DriveClient::new(&token);

        let report = protocol::sync_now(db, &drive, settings.sync_images).await
            .map_err(|e| e.to_string())?;

        if report.skipped {
            Ok("Already up to date".into())
        } else {
            Ok(format!("Synced: pushed {}/{} clips/folders, pulled {}/{}, deleted {}",
                report.pushed_clips, report.pushed_folders,
                report.pulled_clips, report.pulled_folders,
                report.deleted))
        }
    }.await;

    IS_SYNCING.store(false, Ordering::SeqCst);
    result
}

// ── Background auto-sync ──

/// Start the background auto-sync task.
pub fn start_auto_sync(db: Arc<Database>) {
    let (tx, mut rx) = tokio::sync::watch::channel(false);
    *SYNC_ABORT.lock() = Some(tx);

    tokio::spawn(async move {
        loop {
            let settings = get_sync_settings(&db).await;
            if !settings.enabled {
                // Check again in 30 seconds if sync gets enabled
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => continue,
                    _ = rx.changed() => {
                        if *rx.borrow() { break; }
                        continue;
                    }
                }
            }

            let interval = std::time::Duration::from_secs(settings.interval_seconds.max(60));

            tokio::select! {
                _ = tokio::time::sleep(interval) => {},
                _ = rx.changed() => {
                    if *rx.borrow() { break; }
                    continue;
                }
            }

            // Run sync
            match execute_sync(&db).await {
                Ok(msg) => log::info!("SYNC (auto): {}", msg),
                Err(e) => log::warn!("SYNC (auto): Failed — {}", e),
            }
        }
        log::info!("SYNC: Background task stopped");
    });
}

/// Stop the background auto-sync task.
pub fn stop_auto_sync() {
    if let Some(tx) = SYNC_ABORT.lock().take() {
        let _ = tx.send(true);
    }
}

// ── Helpers ──

async fn save_setting(pool: &sqlx::SqlitePool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
        .bind(key).bind(value).execute(pool).await?;
    Ok(())
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, SyncError> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i+2], 16)
            .map_err(|_| SyncError::Encryption("Invalid hex in salt".into())))
        .collect()
}
