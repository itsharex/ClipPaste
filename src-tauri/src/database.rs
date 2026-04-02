use sqlx::SqlitePool;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
    pub images_dir: PathBuf,
}

impl Database {
    pub async fn new(db_path: &str, data_dir: &std::path::Path) -> Self {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .expect("Failed to connect to database");

        // Set per-connection PRAGMAs for performance
        sqlx::query("PRAGMA cache_size = -8000") // 8MB cache
            .execute(&pool).await.ok();
        sqlx::query("PRAGMA temp_store = MEMORY")
            .execute(&pool).await.ok();
        sqlx::query("PRAGMA mmap_size = 67108864") // 64MB mmap
            .execute(&pool).await.ok();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool).await.ok();

        let images_dir = data_dir.join("images");
        std::fs::create_dir_all(&images_dir).ok();

        Self { pool, images_dir }
    }

    async fn get_schema_version(&self) -> i64 {
        // Create version table if not exists
        let _ = sqlx::query("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")
            .execute(&self.pool).await;
        sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(version), 0) FROM schema_version")
            .fetch_one(&self.pool).await.unwrap_or(0)
    }

    async fn set_schema_version(&self, version: i64) {
        if let Err(e) = sqlx::query("DELETE FROM schema_version").execute(&self.pool).await {
            log::error!("Failed to clear schema_version table: {}", e);
        }
        if let Err(e) = sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
            .bind(version).execute(&self.pool).await
        {
            log::error!("Failed to set schema_version to {}: {}", version, e);
        }
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                icon TEXT,
                color TEXT,
                is_system INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS clips (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT NOT NULL UNIQUE,
                clip_type TEXT NOT NULL,
                content BLOB NOT NULL,
                text_preview TEXT,
                content_hash TEXT NOT NULL,
                folder_id INTEGER REFERENCES folders(id),
                is_deleted INTEGER DEFAULT 0,
                source_app TEXT,
                source_icon TEXT,
                metadata TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_accessed DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE INDEX IF NOT EXISTS idx_clips_hash ON clips(content_hash);
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE INDEX IF NOT EXISTS idx_clips_folder ON clips(folder_id);
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE INDEX IF NOT EXISTS idx_clips_created ON clips(created_at);
        "#).execute(&self.pool).await?;

        // idx_clips_deleted_created removed in migration v4 — soft-delete no longer used

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS ignored_apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_name TEXT NOT NULL UNIQUE
            )
        "#).execute(&self.pool).await?;

        // === Version-tracked migrations ===
        let version = self.get_schema_version().await;

        if version < 1 {
            let _ = sqlx::query("ALTER TABLE clips ADD COLUMN last_pasted_at DATETIME DEFAULT NULL")
                .execute(&self.pool).await;
            let _ = sqlx::query("ALTER TABLE folders ADD COLUMN position INTEGER DEFAULT 0")
                .execute(&self.pool).await;
            let _ = sqlx::query("ALTER TABLE clips ADD COLUMN is_pinned INTEGER DEFAULT 0")
                .execute(&self.pool).await;
            self.set_schema_version(1).await;
            log::info!("DB: Applied migration v1 (last_pasted_at, position, is_pinned)");
        }

        if version < 2 {
            let _ = sqlx::query("ALTER TABLE clips ADD COLUMN subtype TEXT DEFAULT NULL")
                .execute(&self.pool).await;
            let _ = sqlx::query("ALTER TABLE clips ADD COLUMN note TEXT DEFAULT NULL")
                .execute(&self.pool).await;
            let _ = sqlx::query("ALTER TABLE clips ADD COLUMN paste_count INTEGER DEFAULT 0")
                .execute(&self.pool).await;
            self.set_schema_version(2).await;
            log::info!("DB: Applied migration v2 (subtype, note, paste_count)");
        }

        if version < 3 {
            let _ = sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_folders_name ON folders(name)")
                .execute(&self.pool).await;
            self.set_schema_version(3).await;
            log::info!("DB: Applied migration v3 (unique folder names)");
        }

        if version < 4 {
            // Drop unused index — soft-delete no longer used, all deletes are hard deletes
            let _ = sqlx::query("DROP INDEX IF EXISTS idx_clips_deleted_created")
                .execute(&self.pool).await;
            // Final cleanup of any remaining soft-deleted rows
            let cleaned: u64 = sqlx::query("DELETE FROM clips WHERE is_deleted = 1")
                .execute(&self.pool).await.map(|r| r.rows_affected()).unwrap_or(0);
            if cleaned > 0 {
                log::info!("DB: Final purge of {} legacy soft-deleted clips", cleaned);
            }
            self.set_schema_version(4).await;
            log::info!("DB: Applied migration v4 (drop unused is_deleted index)");
        }

        // === Migrate image blobs to disk ===
        // Images previously stored as BLOBs in content column are now stored as files.
        // content column will hold just the filename (e.g. "abc123.png").
        self.migrate_images_to_disk().await;

        // Rebuild text_preview for existing clips that have short previews (< 500 chars)
        // This upgrades old 200-char previews to 2000-char previews
        let upgraded: u64 = sqlx::query(r#"
            UPDATE clips SET text_preview = SUBSTR(CAST(content AS TEXT), 1, 2000)
            WHERE clip_type != 'image' AND LENGTH(text_preview) < 500
            AND LENGTH(CAST(content AS TEXT)) > LENGTH(text_preview)
        "#).execute(&self.pool).await.map(|r| r.rows_affected()).unwrap_or(0);
        if upgraded > 0 {
            log::info!("DB: Upgraded text_preview for {} clips (200 → 2000 chars)", upgraded);
        }

       Ok(())
    }

    /// Returns the full path for an image filename
    pub fn image_path(&self, filename: &str) -> PathBuf {
        self.images_dir.join(filename)
    }

    /// Enforce max_items setting — delete oldest non-folder clips exceeding the limit.
    /// Uses a transaction to atomically count + collect image filenames + delete.
    /// Image files are cleaned up after the transaction commits.
    pub async fn enforce_max_items(&self) {
        // Only enforce if user explicitly set max_items in settings
        let max_items: Option<i64> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'max_items'")
            .fetch_optional(&self.pool).await.unwrap_or(None)
            .and_then(|v: String| v.parse().ok());

        let max_items = match max_items {
            Some(v) if v > 0 => v,
            _ => return, // No limit set — unlimited history
        };

        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => { log::error!("enforce_max_items: failed to begin tx: {}", e); return; }
        };

        // Count only unprotected clips (not in folder, not pinned)
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM clips WHERE folder_id IS NULL AND is_pinned = 0"
        ).fetch_one(&mut *tx).await.unwrap_or(0);

        if count <= max_items {
            let _ = tx.commit().await;
            return;
        }

        let excess = count - max_items;
        log::info!("DB: Trimming {} clips exceeding max_items={}", excess, max_items);

        // Collect image filenames before deleting (within same transaction)
        // Only from unprotected clips (not in folder, not pinned)
        let image_clips: Vec<(Vec<u8>,)> = sqlx::query_as(
            "SELECT content FROM clips WHERE folder_id IS NULL AND is_pinned = 0 AND clip_type = 'image'
             ORDER BY created_at ASC LIMIT ?"
        ).bind(excess).fetch_all(&mut *tx).await.unwrap_or_default();

        // Delete oldest unprotected clips (folder + pinned items are safe)
        if let Err(e) = sqlx::query(
            "DELETE FROM clips WHERE id IN (
                SELECT id FROM clips WHERE folder_id IS NULL AND is_pinned = 0
                ORDER BY created_at ASC LIMIT ?
            )"
        ).bind(excess).execute(&mut *tx).await {
            log::error!("Failed to trim excess clips: {}", e);
            let _ = tx.rollback().await;
            return;
        }

        if let Err(e) = tx.commit().await {
            log::error!("enforce_max_items: commit failed: {}", e);
            return;
        }

        // Clean up image files after successful commit
        for (content,) in &image_clips {
            let filename = String::from_utf8_lossy(content).to_string();
            let path = self.images_dir.join(&filename);
            if path.exists() { let _ = std::fs::remove_file(&path); }
        }
    }

    /// Delete clips older than auto_delete_days (only unprotected: not in folder, not pinned).
    pub async fn enforce_auto_delete(&self) {
        let days: Option<i64> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'auto_delete_days'")
            .fetch_optional(&self.pool).await.unwrap_or(None)
            .and_then(|v: String| v.parse().ok());

        let days = match days {
            Some(v) if v > 0 => v,
            _ => return, // 0 or not set = disabled
        };

        // Collect image filenames before deleting
        let image_clips: Vec<(Vec<u8>,)> = sqlx::query_as(
            "SELECT content FROM clips WHERE folder_id IS NULL AND is_pinned = 0 AND clip_type = 'image'
             AND created_at < datetime('now', '-' || ? || ' days')"
        ).bind(days).fetch_all(&self.pool).await.unwrap_or_default();

        let result = sqlx::query(
            "DELETE FROM clips WHERE folder_id IS NULL AND is_pinned = 0
             AND created_at < datetime('now', '-' || ? || ' days')"
        ).bind(days).execute(&self.pool).await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                log::info!("DB: Auto-deleted {} clips older than {} days", r.rows_affected(), days);
                for (content,) in &image_clips {
                    let filename = String::from_utf8_lossy(content).to_string();
                    let path = self.images_dir.join(&filename);
                    if path.exists() { let _ = std::fs::remove_file(&path); }
                }
            }
            Err(e) => log::error!("enforce_auto_delete failed: {}", e),
            _ => {}
        }
    }

    /// Clean up orphan image files (files in images/ that have no matching clip)
    pub async fn cleanup_orphan_images(&self) {
        let entries = match std::fs::read_dir(&self.images_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        // Load all image filenames from DB in one query
        let db_files: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT CAST(content AS TEXT) FROM clips WHERE clip_type = 'image'"
        ).fetch_all(&self.pool).await.unwrap_or_default().into_iter().collect();

        let mut orphans = 0u64;
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if !db_files.contains(&filename) {
                let _ = std::fs::remove_file(entry.path());
                orphans += 1;
            }
        }

        if orphans > 0 {
            log::info!("DB: Cleaned up {} orphan image files", orphans);
        }
    }

    /// Migrate existing image BLOBs from the database to disk files.
    /// After migration, the `content` column holds just the filename.
    async fn migrate_images_to_disk(&self) {
        // Find image clips whose content is larger than a filename would be (> 260 bytes = raw BLOB)
        let rows: Vec<(i64, Vec<u8>, String)> = sqlx::query_as(
            "SELECT id, content, content_hash FROM clips WHERE clip_type = 'image' AND LENGTH(content) > 260"
        ).fetch_all(&self.pool).await.unwrap_or_default();

        if rows.is_empty() { return; }

        log::info!("DB: Migrating {} image BLOBs to disk...", rows.len());
        let mut migrated = 0u64;

        for (id, blob, hash) in &rows {
            let filename = format!("{}.png", hash);
            let file_path = self.images_dir.join(&filename);

            // Write blob to file (skip if already exists)
            if !file_path.exists() {
                if let Err(e) = std::fs::write(&file_path, blob) {
                    log::error!("DB: Failed to write image file {:?}: {}", file_path, e);
                    continue;
                }
            }

            // Update DB: replace blob with just the filename
            if let Err(e) = sqlx::query("UPDATE clips SET content = ? WHERE id = ?")
                .bind(filename.as_bytes())
                .bind(id)
                .execute(&self.pool)
                .await
            {
                log::error!("DB: Failed to update clip {} after image migration: {}", id, e);
                continue;
            }
            migrated += 1;
        }

        log::info!("DB: Migrated {} image BLOBs to disk.", migrated);

        // VACUUM to reclaim disk space after removing large BLOBs
        log::info!("DB: Running VACUUM to reclaim space...");
        if let Err(e) = sqlx::query("VACUUM").execute(&self.pool).await {
            log::warn!("DB: VACUUM failed (non-fatal): {}", e);
        } else {
            log::info!("DB: VACUUM complete.");
        }
    }

    pub async fn add_ignored_app(&self, app_name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR IGNORE INTO ignored_apps (app_name) VALUES (?)")
            .bind(app_name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_ignored_app(&self, app_name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM ignored_apps WHERE app_name = ?")
            .bind(app_name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_ignored_apps(&self) -> Result<Vec<String>, sqlx::Error> {
        let apps = sqlx::query_scalar::<_, String>("SELECT app_name FROM ignored_apps ORDER BY app_name")
            .fetch_all(&self.pool)
            .await?;
        log::info!("DB: Ignored apps: {:?}", apps);
        Ok(apps)
    }

    pub async fn is_app_ignored(&self, app_name: &str) -> Result<bool, sqlx::Error> {
        // Case-insensitive check might be better for Windows exe names
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ignored_apps WHERE LOWER(app_name) = LOWER(?)")
            .bind(app_name)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, sqlx::Error> {
        let value = sqlx::query_scalar(r#"
            SELECT value FROM settings WHERE key = ?
        "#)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(value)
    }
}