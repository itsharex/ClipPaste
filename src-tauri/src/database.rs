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
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options)
            .await
            .expect("Failed to connect to database");

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
        let _ = sqlx::query("DELETE FROM schema_version").execute(&self.pool).await;
        let _ = sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
            .bind(version).execute(&self.pool).await;
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

        sqlx::query(r#"
            CREATE INDEX IF NOT EXISTS idx_clips_deleted_created ON clips(is_deleted, created_at);
        "#).execute(&self.pool).await?;

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

        // Clean up legacy soft-deleted rows (hard delete now)
        let cleaned: u64 = sqlx::query("DELETE FROM clips WHERE is_deleted = 1")
            .execute(&self.pool).await.map(|r| r.rows_affected()).unwrap_or(0);
        if cleaned > 0 {
            log::info!("DB: Purged {} legacy soft-deleted clips", cleaned);
        }

        // === Migrate image blobs to disk ===
        // Images previously stored as BLOBs in content column are now stored as files.
        // content column will hold just the filename (e.g. "abc123.png").
        self.migrate_images_to_disk().await;

        // === FTS5 Full-Text Search ===
        // Create FTS5 virtual table for fast text search
        sqlx::query(r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS clips_fts USING fts5(
                uuid UNINDEXED,
                text_content,
                content=''
            )
        "#).execute(&self.pool).await?;

        // Populate FTS5 from existing text clips (skip if already populated)
        let fts_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clips_fts")
            .fetch_one(&self.pool).await.unwrap_or(0);
        let clip_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM clips WHERE clip_type != 'image' AND is_deleted = 0"
        ).fetch_one(&self.pool).await.unwrap_or(0);

        if fts_count == 0 && clip_count > 0 {
            log::info!("DB: Populating FTS5 index from {} existing text clips...", clip_count);
            sqlx::query(r#"
                INSERT INTO clips_fts(uuid, text_content)
                SELECT uuid, CAST(content AS TEXT) FROM clips
                WHERE clip_type != 'image'             "#).execute(&self.pool).await?;
            log::info!("DB: FTS5 index populated.");
        }

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
    /// Also cleans up image files for deleted image clips.
    pub async fn enforce_max_items(&self) {
        let max_items: i64 = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'max_items'")
            .fetch_optional(&self.pool).await.unwrap_or(None)
            .and_then(|v: String| v.parse().ok())
            .unwrap_or(1000);

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clips WHERE folder_id IS NULL")
            .fetch_one(&self.pool).await.unwrap_or(0);

        if count <= max_items { return; }

        let excess = count - max_items;
        log::info!("DB: Trimming {} clips exceeding max_items={}", excess, max_items);

        // Get image filenames for clips about to be deleted
        let image_clips: Vec<(Vec<u8>,)> = sqlx::query_as(
            "SELECT content FROM clips WHERE folder_id IS NULL AND clip_type = 'image'
             ORDER BY created_at ASC LIMIT ?"
        ).bind(excess).fetch_all(&self.pool).await.unwrap_or_default();

        for (content,) in &image_clips {
            let filename = String::from_utf8_lossy(content).to_string();
            let path = self.images_dir.join(&filename);
            if path.exists() { let _ = std::fs::remove_file(&path); }
        }

        // Delete oldest non-folder clips
        let _ = sqlx::query(
            "DELETE FROM clips WHERE id IN (
                SELECT id FROM clips WHERE folder_id IS NULL
                ORDER BY created_at ASC LIMIT ?
            )"
        ).bind(excess).execute(&self.pool).await;
    }

    /// Clean up orphan image files (files in images/ that have no matching clip)
    pub async fn cleanup_orphan_images(&self) {
        let entries = match std::fs::read_dir(&self.images_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        let mut orphans = 0u64;
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            let exists: bool = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM clips WHERE clip_type = 'image' AND CAST(content AS TEXT) = ?"
            ).bind(&filename).fetch_one(&self.pool).await.unwrap_or(0) > 0;

            if !exists {
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