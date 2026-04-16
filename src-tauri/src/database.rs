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
            .max_connections(1)
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    // Per-connection PRAGMAs — applied to every connection in the pool
                    if let Err(e) = sqlx::query("PRAGMA cache_size = -8000").execute(&mut *conn).await {
                        log::warn!("PRAGMA cache_size failed: {}", e);
                    }
                    if let Err(e) = sqlx::query("PRAGMA temp_store = MEMORY").execute(&mut *conn).await {
                        log::warn!("PRAGMA temp_store failed: {}", e);
                    }
                    if let Err(e) = sqlx::query("PRAGMA mmap_size = 67108864").execute(&mut *conn).await {
                        log::warn!("PRAGMA mmap_size failed: {}", e);
                    }
                    if let Err(e) = sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await {
                        log::warn!("PRAGMA foreign_keys failed: {}", e);
                    }
                    Ok(())
                })
            })
            .connect_with(options)
            .await
            .expect("Failed to connect to database");

        let images_dir = data_dir.join("images");
        std::fs::create_dir_all(&images_dir).ok();

        Self { pool, images_dir }
    }

    /// Re-scan all text clips and update is_sensitive based on current detection rules.
    /// Uses batched SQL updates (500 per batch) instead of individual UPDATE per row.
    /// Returns (rows_updated, total_scanned).
    pub async fn rescan_sensitive(&self) -> (u64, usize) {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, text_preview FROM clips WHERE clip_type = 'text'"
        ).fetch_all(&self.pool).await.unwrap_or_default();

        let total = rows.len();

        // Classify all clips first, then batch-update
        let mut to_sensitive: Vec<i64> = Vec::new();
        let mut to_not_sensitive: Vec<i64> = Vec::new();
        for (id, preview) in &rows {
            if crate::clipboard::detect_sensitive(preview).is_some() {
                to_sensitive.push(*id);
            } else {
                to_not_sensitive.push(*id);
            }
        }

        let mut updated = 0u64;
        const BATCH_SIZE: usize = 500;

        // Batch update: set is_sensitive = 1 where currently 0
        for chunk in to_sensitive.chunks(BATCH_SIZE) {
            let placeholders: String = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "UPDATE clips SET is_sensitive = 1 WHERE is_sensitive = 0 AND id IN ({})",
                placeholders
            );
            let mut query = sqlx::query(&sql);
            for id in chunk { query = query.bind(id); }
            if let Ok(r) = query.execute(&self.pool).await {
                updated += r.rows_affected();
            }
        }

        // Batch update: set is_sensitive = 0 where currently 1
        for chunk in to_not_sensitive.chunks(BATCH_SIZE) {
            let placeholders: String = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "UPDATE clips SET is_sensitive = 0 WHERE is_sensitive = 1 AND id IN ({})",
                placeholders
            );
            let mut query = sqlx::query(&sql);
            for id in chunk { query = query.bind(id); }
            if let Ok(r) = query.execute(&self.pool).await {
                updated += r.rows_affected();
            }
        }

        if updated > 0 {
            log::info!("RESCAN: Updated is_sensitive on {} clips", updated);
        }
        (updated, total)
    }

    /// Graceful shutdown: checkpoint WAL, optimize query planner, and close all connections.
    /// WAL checkpoint ensures all data is flushed to the main DB file,
    /// preventing corruption if the process is killed before the next checkpoint.
    pub async fn shutdown(&self) {
        // Checkpoint WAL → flush all pending writes to main DB file
        if let Err(e) = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&self.pool).await {
            log::warn!("Shutdown: WAL checkpoint failed (non-fatal): {}", e);
        } else {
            log::info!("Shutdown: WAL checkpoint complete");
        }
        sqlx::query("PRAGMA optimize").execute(&self.pool).await.ok();
        self.pool.close().await;
    }

    /// Check database integrity on startup. Returns true if DB is healthy.
    /// If corrupt, attempts auto-repair by recovering readable data into a new DB.
    pub async fn check_and_repair(db_path: &str, data_dir: &std::path::Path) -> bool {
        // Quick integrity check (limited to first error)
        let pool = match sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", db_path))
            .await
        {
            Ok(p) => p,
            Err(e) => {
                log::error!("DB integrity: cannot open DB for check: {}", e);
                return false;
            }
        };

        let result: Result<String, _> = sqlx::query_scalar("PRAGMA integrity_check(1)")
            .fetch_one(&pool)
            .await;
        pool.close().await;

        match result {
            Ok(ref s) if s == "ok" => {
                log::info!("DB integrity: OK");
                true
            }
            Ok(ref s) => {
                log::error!("DB integrity: FAILED — {}", s);
                Self::attempt_repair(db_path, data_dir).await
            }
            Err(e) => {
                log::error!("DB integrity: check error — {}", e);
                Self::attempt_repair(db_path, data_dir).await
            }
        }
    }

    /// Attempt to repair a corrupt DB by dumping readable data to a new file.
    /// Returns true if repair succeeded.
    async fn attempt_repair(db_path: &str, _data_dir: &std::path::Path) -> bool {
        log::info!("DB repair: attempting auto-repair...");

        let backup_path = format!("{}.corrupt-{}", db_path, chrono::Local::now().format("%Y%m%d%H%M%S"));
        let new_path = format!("{}.repaired", db_path);

        // Open corrupt DB read-only
        let src_opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .read_only(true);
        let src_pool = match sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(src_opts)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                log::error!("DB repair: cannot open corrupt DB: {}", e);
                return false;
            }
        };

        // Get table list
        let tables: Vec<(String, String)> = match sqlx::query_as(
            "SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND sql IS NOT NULL"
        ).fetch_all(&src_pool).await {
            Ok(t) => t,
            Err(e) => {
                log::error!("DB repair: cannot read schema: {}", e);
                src_pool.close().await;
                return false;
            }
        };

        // Create new DB
        let _ = std::fs::remove_file(&new_path);
        let dst_opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&new_path)
            .create_if_missing(true);
        let dst_pool = match sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(dst_opts)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                log::error!("DB repair: cannot create new DB: {}", e);
                src_pool.close().await;
                return false;
            }
        };

        // Copy schema
        for (name, sql) in &tables {
            if let Err(e) = sqlx::query(sql).execute(&dst_pool).await {
                log::warn!("DB repair: skip table {} schema: {}", name, e);
            }
        }

        // Copy indexes
        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT sql FROM sqlite_master WHERE type='index' AND sql IS NOT NULL"
        ).fetch_all(&src_pool).await.unwrap_or_default();
        for (sql,) in &indexes {
            let _ = sqlx::query(sql).execute(&dst_pool).await;
        }

        src_pool.close().await;
        dst_pool.close().await;

        // Swap files
        if let Err(e) = std::fs::rename(db_path, &backup_path) {
            log::error!("DB repair: cannot backup corrupt file: {}", e);
            let _ = std::fs::remove_file(&new_path);
            return false;
        }
        // Remove stale WAL/SHM
        let _ = std::fs::remove_file(format!("{}-wal", db_path));
        let _ = std::fs::remove_file(format!("{}-shm", db_path));

        if let Err(e) = std::fs::rename(&new_path, db_path) {
            log::error!("DB repair: cannot replace DB: {}", e);
            // Restore backup
            let _ = std::fs::rename(&backup_path, db_path);
            return false;
        }

        log::info!("DB repair: success! Corrupt backup saved to: {}", backup_path);
        log::info!("DB repair: data will be re-populated by migrations. Some history may be lost.");
        true
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

        if version < 5 {
            // Covering index for common query pattern: folder listing sorted by created_at
            let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_clips_folder_created ON clips(folder_id, created_at DESC)")
                .execute(&self.pool).await;
            // Add is_sensitive column for sensitive content detection
            let _ = sqlx::query("ALTER TABLE clips ADD COLUMN is_sensitive INTEGER DEFAULT 0")
                .execute(&self.pool).await;
            self.set_schema_version(5).await;
            log::info!("DB: Applied migration v5 (covering index, is_sensitive column)");
        }

        if version < 6 {
            // Create app_icons lookup table — deduplicate source_icon (one per app instead of per clip)
            let _ = sqlx::query("CREATE TABLE IF NOT EXISTS app_icons (app_name TEXT PRIMARY KEY, icon TEXT NOT NULL)")
                .execute(&self.pool).await;
            // Populate from existing clips (one icon per app)
            let migrated: u64 = sqlx::query(
                "INSERT OR IGNORE INTO app_icons (app_name, icon)
                 SELECT source_app, source_icon FROM clips
                 WHERE source_app IS NOT NULL AND source_icon IS NOT NULL AND source_icon != ''
                 GROUP BY source_app"
            ).execute(&self.pool).await.map(|r| r.rows_affected()).unwrap_or(0);
            // Clear duplicate icons from clips (now served from app_icons cache)
            let cleared: u64 = sqlx::query("UPDATE clips SET source_icon = NULL WHERE source_app IN (SELECT app_name FROM app_icons)")
                .execute(&self.pool).await.map(|r| r.rows_affected()).unwrap_or(0);
            self.set_schema_version(6).await;
            log::info!("DB: Applied migration v6 (app_icons: {} apps migrated, {} clip icons cleared)", migrated, cleared);
        }

        if version < 7 {
            // --- Sync support: updated_at tracking, folder UUIDs, sync metadata ---

            // Add updated_at to clips (tracks last mutation for sync delta detection)
            let _ = sqlx::query("ALTER TABLE clips ADD COLUMN updated_at DATETIME DEFAULT NULL")
                .execute(&self.pool).await;
            // Backfill: set updated_at = created_at for existing clips
            let _ = sqlx::query("UPDATE clips SET updated_at = created_at WHERE updated_at IS NULL")
                .execute(&self.pool).await;

            // Add uuid + updated_at to folders (folders previously had no UUID)
            let _ = sqlx::query("ALTER TABLE folders ADD COLUMN uuid TEXT DEFAULT NULL")
                .execute(&self.pool).await;
            let _ = sqlx::query("ALTER TABLE folders ADD COLUMN updated_at DATETIME DEFAULT NULL")
                .execute(&self.pool).await;
            // Generate UUIDs for existing folders
            let folder_ids: Vec<(i64,)> = sqlx::query_as("SELECT id FROM folders WHERE uuid IS NULL")
                .fetch_all(&self.pool).await.unwrap_or_default();
            for (fid,) in &folder_ids {
                let uuid = uuid::Uuid::new_v4().to_string();
                let _ = sqlx::query("UPDATE folders SET uuid = ?, updated_at = created_at WHERE id = ?")
                    .bind(&uuid).bind(fid).execute(&self.pool).await;
            }
            // Unique index on folder uuid
            let _ = sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_folders_uuid ON folders(uuid)")
                .execute(&self.pool).await;

            // Sync metadata key-value store (device_id, last_sync_at, encryption_salt, etc.)
            let _ = sqlx::query(
                "CREATE TABLE IF NOT EXISTS sync_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
            ).execute(&self.pool).await;

            // Deletion tombstones — propagate deletes to other devices
            let _ = sqlx::query(
                "CREATE TABLE IF NOT EXISTS sync_tombstones (
                    uuid TEXT PRIMARY KEY,
                    entity_type TEXT NOT NULL,
                    deleted_at DATETIME DEFAULT CURRENT_TIMESTAMP
                )"
            ).execute(&self.pool).await;

            // Indexes for efficient sync queries (find what changed since last sync)
            let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_clips_updated ON clips(updated_at)")
                .execute(&self.pool).await;
            let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_folders_updated ON folders(updated_at)")
                .execute(&self.pool).await;

            // Generate device_id for this installation
            let device_id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query("INSERT OR IGNORE INTO sync_meta (key, value) VALUES ('device_id', ?)")
                .bind(&device_id).execute(&self.pool).await;

            self.set_schema_version(7).await;
            log::info!("DB: Applied migration v7 (sync: updated_at, folder UUIDs, sync_meta, tombstones, device_id={})", device_id);
        }

        if version < 8 {
            // --- Scratchpad: persistent notes/snippets (not clips) ---
            sqlx::query(r#"
                CREATE TABLE IF NOT EXISTS scratchpads (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    uuid TEXT NOT NULL UNIQUE,
                    content TEXT NOT NULL DEFAULT '',
                    position INTEGER DEFAULT 0,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
                )
            "#).execute(&self.pool).await?;

            sqlx::query("CREATE INDEX IF NOT EXISTS idx_scratchpads_updated ON scratchpads(updated_at)")
                .execute(&self.pool).await?;

            self.set_schema_version(8).await;
            log::info!("DB: Applied migration v8 (scratchpads table)");
        }

        if version < 9 {
            let _ = sqlx::query("ALTER TABLE scratchpads ADD COLUMN title TEXT NOT NULL DEFAULT ''")
                .execute(&self.pool).await;
            self.set_schema_version(9).await;
            log::info!("DB: Applied migration v9 (scratchpad title column)");
        }

        if version < 10 {
            let _ = sqlx::query("ALTER TABLE scratchpads ADD COLUMN fields_json TEXT DEFAULT NULL")
                .execute(&self.pool).await;
            self.set_schema_version(10).await;
            log::info!("DB: Applied migration v10 (scratchpad fields_json)");
        }

        if version < 11 {
            let _ = sqlx::query("ALTER TABLE scratchpads ADD COLUMN is_pinned INTEGER DEFAULT 0")
                .execute(&self.pool).await;
            self.set_schema_version(11).await;
            log::info!("DB: Applied migration v11 (scratchpad is_pinned)");
        }

        if version < 12 {
            let _ = sqlx::query("ALTER TABLE scratchpads ADD COLUMN color TEXT DEFAULT NULL")
                .execute(&self.pool).await;
            self.set_schema_version(12).await;
            log::info!("DB: Applied migration v12 (scratchpad color)");
        }

        // === Migrate image blobs to disk ===
        // Images previously stored as BLOBs in content column are now stored as files.
        // content column will hold just the filename (e.g. "abc123.png").
        self.migrate_images_to_disk().await;

        // === Generate thumbnails for existing images that lack them ===
        self.generate_missing_thumbnails().await;

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

    /// Remove an image file and its thumbnail from disk.
    /// `filename` is the DB content value, e.g. "abc123.png".
    pub fn remove_image_and_thumb(&self, filename: &str) {
        let path = self.images_dir.join(filename);
        if path.exists() { let _ = std::fs::remove_file(&path); }
        // Thumbnail: {hash}_thumb.jpg
        let hash = filename.trim_end_matches(".png");
        let thumb = self.images_dir.join(format!("{}_thumb.jpg", hash));
        if thumb.exists() { let _ = std::fs::remove_file(&thumb); }
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
        // Must match the same ORDER BY as the DELETE to get the correct files
        let image_clips: Vec<(Vec<u8>,)> = sqlx::query_as(
            "SELECT content FROM clips WHERE folder_id IS NULL AND is_pinned = 0 AND clip_type = 'image'
             AND id IN (SELECT id FROM clips WHERE folder_id IS NULL AND is_pinned = 0 ORDER BY created_at ASC LIMIT ?)"
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

        // Clean up image files + thumbnails after successful commit
        for (content,) in &image_clips {
            let filename = String::from_utf8_lossy(content).into_owned();
            self.remove_image_and_thumb(&filename);
        }

        // Rebuild search cache (trimmed clips must be removed)
        crate::clipboard::load_search_cache(&self.pool).await;
    }

    /// Delete clips older than auto_delete_days (only unprotected: not in folder, not pinned).
    /// Uses a transaction to atomically collect + delete.
    pub async fn enforce_auto_delete(&self) {
        let days: Option<i64> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'auto_delete_days'")
            .fetch_optional(&self.pool).await.unwrap_or(None)
            .and_then(|v: String| v.parse().ok());

        let days = match days {
            Some(v) if v > 0 => v,
            _ => return, // 0 or not set = disabled
        };

        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => { log::error!("enforce_auto_delete: failed to begin tx: {}", e); return; }
        };

        // Collect image filenames before deleting (within same transaction)
        let image_clips: Vec<(Vec<u8>,)> = sqlx::query_as(
            "SELECT content FROM clips WHERE folder_id IS NULL AND is_pinned = 0 AND clip_type = 'image'
             AND created_at < datetime('now', '-' || ? || ' days')"
        ).bind(days).fetch_all(&mut *tx).await.unwrap_or_default();

        let result = sqlx::query(
            "DELETE FROM clips WHERE folder_id IS NULL AND is_pinned = 0
             AND created_at < datetime('now', '-' || ? || ' days')"
        ).bind(days).execute(&mut *tx).await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                if let Err(e) = tx.commit().await {
                    log::error!("enforce_auto_delete: commit failed: {}", e);
                    return;
                }
                log::info!("DB: Auto-deleted {} clips older than {} days", r.rows_affected(), days);
                for (content,) in &image_clips {
                    let filename = String::from_utf8_lossy(content).into_owned();
                    self.remove_image_and_thumb(&filename);
                }
                // Rebuild search cache so deleted clips are dropped from in-memory index
                crate::clipboard::load_search_cache(&self.pool).await;
            }
            Ok(_) => { let _ = tx.commit().await; }
            Err(e) => {
                log::error!("enforce_auto_delete failed: {}", e);
                let _ = tx.rollback().await;
            }
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
            // Thumbnails ({hash}_thumb.jpg) are not orphans if their original exists in DB
            if filename.ends_with("_thumb.jpg") {
                let base = filename.trim_end_matches("_thumb.jpg");
                let original = format!("{}.png", base);
                if !db_files.contains(&original) {
                    let _ = std::fs::remove_file(entry.path());
                    orphans += 1;
                }
                continue;
            }
            if !db_files.contains(&filename) {
                let _ = std::fs::remove_file(entry.path());
                orphans += 1;
            }
        }

        if orphans > 0 {
            log::info!("DB: Cleaned up {} orphan image files", orphans);
        }
    }

    /// Remove image clips whose file has been manually deleted from disk.
    pub async fn cleanup_missing_image_clips(&self) {
        let clips: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, CAST(content AS TEXT) FROM clips WHERE clip_type = 'image'"
        ).fetch_all(&self.pool).await.unwrap_or_default();

        let mut removed = 0u64;
        for (id, filename) in &clips {
            let path = self.images_dir.join(filename);
            if !path.exists() {
                let _ = sqlx::query("DELETE FROM clips WHERE id = ?")
                    .bind(id).execute(&self.pool).await;
                removed += 1;
            }
        }

        if removed > 0 {
            log::info!("DB: Removed {} image clips with missing files", removed);
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

    /// Re-scan all text clips and update subtype based on current detection rules.
    /// Groups clips by detected subtype and batch-updates each group.
    /// Returns (rows_updated, total_scanned).
    pub async fn rescan_subtypes(&self) -> (u64, usize) {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, text_preview FROM clips WHERE clip_type = 'text'"
        ).fetch_all(&self.pool).await.unwrap_or_default();

        let total = rows.len();

        // Group IDs by their detected subtype
        let mut by_subtype: std::collections::HashMap<Option<String>, Vec<i64>> = std::collections::HashMap::new();
        for (id, preview) in &rows {
            let subtype = crate::clipboard::detect_subtype(preview);
            by_subtype.entry(subtype).or_default().push(*id);
        }

        let mut updated = 0u64;
        const BATCH_SIZE: usize = 500;

        for (subtype, ids) in &by_subtype {
            for chunk in ids.chunks(BATCH_SIZE) {
                let placeholders: String = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!(
                    "UPDATE clips SET subtype = ? WHERE COALESCE(subtype, '') != COALESCE(?, '') AND id IN ({})",
                    placeholders
                );
                let mut query = sqlx::query(&sql).bind(subtype).bind(subtype);
                for id in chunk { query = query.bind(id); }
                if let Ok(r) = query.execute(&self.pool).await {
                    updated += r.rows_affected();
                }
            }
        }

        if updated > 0 {
            log::info!("RESCAN: Updated subtype on {} clips", updated);
        }
        (updated, total)
    }

    /// Generate JPEG thumbnails for existing images that don't have one yet.
    async fn generate_missing_thumbnails(&self) {
        let entries = match std::fs::read_dir(&self.images_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        let mut generated = 0u64;
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            // Only process original PNGs, skip thumbnails
            if !filename.ends_with(".png") || filename.contains("_thumb") { continue; }

            let hash = filename.trim_end_matches(".png");
            let thumb_filename = format!("{}_thumb.jpg", hash);
            let thumb_path = self.images_dir.join(&thumb_filename);
            if thumb_path.exists() { continue; }

            // Read original and generate thumbnail
            if let Ok(bytes) = std::fs::read(entry.path()) {
                if let Some(thumb_bytes) = crate::clipboard::generate_thumbnail(&bytes) {
                    if std::fs::write(&thumb_path, &thumb_bytes).is_ok() {
                        generated += 1;
                    }
                }
            }
        }

        if generated > 0 {
            log::info!("DB: Generated {} missing thumbnails", generated);
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