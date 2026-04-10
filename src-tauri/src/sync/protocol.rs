use super::drive::DriveClient;
use super::error::SyncError;
use super::models::*;
use crate::database::Database;

const STATE_FILE: &str = "sync_state.json";
const DELTA_PREFIX: &str = "delta_";
const MAX_DELTAS_BEFORE_COMPACT: usize = 50;

/// Full snapshot — uploaded once, then compacted periodically.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct SyncState {
    pub(crate) clips: Vec<SyncClip>,
    pub(crate) folders: Vec<SyncFolder>,
    pub(crate) tombstones: Vec<Tombstone>,
    pub(crate) device_id: String,
    pub(crate) updated_at: String,
}

/// Small delta — only contains changes since last sync.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SyncDelta {
    pub(crate) clips: Vec<SyncClip>,
    pub(crate) folders: Vec<SyncFolder>,
    pub(crate) tombstones: Vec<Tombstone>,
    pub(crate) device_id: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub pushed_clips: u64,
    pub pushed_folders: u64,
    pub pulled_clips: u64,
    pub pulled_folders: u64,
    pub deleted: u64,
    pub skipped: bool,
    pub errors: Vec<String>,
}

/// Run a sync cycle using delta approach.
pub async fn sync_now(
    db: &Database,
    drive: &DriveClient,
    sync_images: bool,
) -> Result<SyncReport, SyncError> {
    let mut report = SyncReport::default();

    let device_id = super::get_device_id(db).await
        .unwrap_or_else(|| "unknown".to_string());
    let last_sync_at = db.get_setting("sync_last_sync_at").await
        .map_err(|e| SyncError::Database(e.to_string()))?
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

    // 1. Check if state file exists (first sync?)
    let has_state = drive.find_file_by_name(STATE_FILE).await?.is_some();

    if !has_state {
        // First sync: upload full state
        log::info!("SYNC: First sync — uploading full state");
        let state = build_full_state(db, &device_id, sync_images).await?;
        let json = serde_json::to_vec(&state)?;
        drive.upsert_file(STATE_FILE, &json, "application/json").await?;
        report.pushed_clips = state.clips.len() as u64;
        report.pushed_folders = state.folders.len() as u64;

        if sync_images {
            push_new_images(db, drive, &std::collections::HashSet::new(), &mut report).await?;
        }

        let now = chrono::Utc::now().to_rfc3339();
        save_setting(&db.pool, "sync_last_sync_at", &now).await?;
        log::info!("SYNC: First sync complete — {} clips, {} folders", report.pushed_clips, report.pushed_folders);
        return Ok(report);
    }

    // 2. List delta files on Drive (lightweight call)
    let delta_files = drive.list_files(Some(DELTA_PREFIX), None).await?;
    let remote_deltas: Vec<_> = delta_files.iter()
        .filter(|f| f.name.starts_with(DELTA_PREFIX) && f.name != format!("{}_{}.json", DELTA_PREFIX, device_id))
        .collect();

    // 3. Pull: download and apply remote deltas
    for delta_file in &remote_deltas {
        let data = drive.download_file(&delta_file.id).await?;
        let delta: SyncDelta = match serde_json::from_slice(&data) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("SYNC: Failed to parse delta {}: {}", delta_file.name, e);
                continue;
            }
        };
        // Skip our own deltas
        if delta.device_id == device_id { continue; }

        apply_delta(db, &delta, sync_images, drive, &mut report).await?;
    }

    // 4. Check local changes
    let changed_clips = get_changed_clips(db, &last_sync_at, sync_images).await?;
    let changed_folders = get_changed_folders(db, &last_sync_at).await?;
    let tombstones = get_tombstones(db).await?;

    let has_local_changes = !changed_clips.is_empty() || !changed_folders.is_empty() || !tombstones.is_empty();

    if has_local_changes {
        // 5. Push: upload small delta with only our changes
        let delta = SyncDelta {
            clips: changed_clips,
            folders: changed_folders,
            tombstones,
            device_id: device_id.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        report.pushed_clips = delta.clips.len() as u64;
        report.pushed_folders = delta.folders.len() as u64;

        let delta_name = format!("{}_{}.json", DELTA_PREFIX, device_id);
        let json = serde_json::to_vec(&delta)?;
        log::info!("SYNC: Uploading delta ({} bytes, {} clips, {} folders)",
            json.len(), delta.clips.len(), delta.folders.len());
        drive.upsert_file(&delta_name, &json, "application/json").await?;

        // Push new images
        if sync_images {
            let remote_hashes = get_remote_image_hashes(drive).await?;
            push_new_images(db, drive, &remote_hashes, &mut report).await?;
        }
    } else if remote_deltas.is_empty() {
        log::info!("SYNC: No changes anywhere, skipping");
        report.skipped = true;
    }

    // 6. Compact if too many deltas
    let total_deltas = delta_files.iter().filter(|f| f.name.starts_with(DELTA_PREFIX)).count();
    if total_deltas >= MAX_DELTAS_BEFORE_COMPACT {
        log::info!("SYNC: Compacting {} deltas into full state", total_deltas);
        compact(db, drive, &device_id, sync_images, &delta_files).await?;
    }

    // 7. Update last_sync_at
    let now = chrono::Utc::now().to_rfc3339();
    save_setting(&db.pool, "sync_last_sync_at", &now).await?;
    super::cleanup_tombstones(db).await.ok();

    log::info!("SYNC: Complete — pushed {}/{}, pulled {}/{}, deleted {}",
        report.pushed_clips, report.pushed_folders, report.pulled_clips, report.pulled_folders, report.deleted);

    Ok(report)
}

// ── Build state from DB ──

pub(crate) async fn build_full_state(db: &Database, device_id: &str, sync_images: bool) -> Result<SyncState, SyncError> {
    let clips = get_all_clips(db, sync_images).await?;
    let folders = get_all_folders(db).await?;
    let tombstones = get_tombstones(db).await?;

    Ok(SyncState {
        clips,
        folders,
        tombstones,
        device_id: device_id.to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    })
}

async fn get_all_clips(db: &Database, sync_images: bool) -> Result<Vec<SyncClip>, SyncError> {
    let rows: Vec<(String, String, Vec<u8>, String, String, Option<i64>, Option<String>,
                    Option<String>, Option<String>, Option<String>, i64, bool, bool,
                    String, String)> =
        sqlx::query_as(
            "SELECT uuid, clip_type, content, text_preview, content_hash, folder_id,
                    source_app, metadata, subtype, note, paste_count, is_pinned, is_sensitive,
                    created_at, COALESCE(updated_at, created_at)
             FROM clips ORDER BY created_at DESC"
        ).fetch_all(&db.pool).await?;

    let mut clips = Vec::with_capacity(rows.len());
    for (uuid, clip_type, content, preview, hash, folder_id, source_app,
         metadata, subtype, note, paste_count, is_pinned, is_sensitive,
         created_at, updated_at) in rows {

        if clip_type == "image" && !sync_images { continue; }

        let folder_uuid = resolve_folder_uuid(db, folder_id).await?;
        let text_content = if clip_type != "image" {
            Some(String::from_utf8_lossy(&content).into_owned())
        } else { None };

        clips.push(SyncClip {
            uuid, clip_type, text_preview: preview, content_hash: hash,
            folder_uuid, source_app, metadata, subtype, note,
            paste_count, is_pinned, is_sensitive, created_at, updated_at, text_content,
        });
    }
    Ok(clips)
}

async fn get_changed_clips(db: &Database, since: &str, sync_images: bool) -> Result<Vec<SyncClip>, SyncError> {
    let rows: Vec<(String, String, Vec<u8>, String, String, Option<i64>, Option<String>,
                    Option<String>, Option<String>, Option<String>, i64, bool, bool,
                    String, String)> =
        sqlx::query_as(
            "SELECT uuid, clip_type, content, text_preview, content_hash, folder_id,
                    source_app, metadata, subtype, note, paste_count, is_pinned, is_sensitive,
                    created_at, COALESCE(updated_at, created_at)
             FROM clips WHERE COALESCE(updated_at, created_at) > ?"
        ).bind(since).fetch_all(&db.pool).await?;

    let mut clips = Vec::new();
    for (uuid, clip_type, content, preview, hash, folder_id, source_app,
         metadata, subtype, note, paste_count, is_pinned, is_sensitive,
         created_at, updated_at) in rows {

        if clip_type == "image" && !sync_images { continue; }

        let folder_uuid = resolve_folder_uuid(db, folder_id).await?;
        let text_content = if clip_type != "image" {
            Some(String::from_utf8_lossy(&content).into_owned())
        } else { None };

        clips.push(SyncClip {
            uuid, clip_type, text_preview: preview, content_hash: hash,
            folder_uuid, source_app, metadata, subtype, note,
            paste_count, is_pinned, is_sensitive, created_at, updated_at, text_content,
        });
    }
    Ok(clips)
}

async fn get_all_folders(db: &Database) -> Result<Vec<SyncFolder>, SyncError> {
    let rows: Vec<(String, String, Option<String>, Option<String>, i64, String, String)> =
        sqlx::query_as(
            "SELECT uuid, name, icon, color, position, created_at, COALESCE(updated_at, created_at)
             FROM folders WHERE uuid IS NOT NULL"
        ).fetch_all(&db.pool).await?;

    Ok(rows.into_iter().map(|(uuid, name, icon, color, position, created_at, updated_at)| {
        SyncFolder { uuid, name, icon, color, position, created_at, updated_at }
    }).collect())
}

async fn get_changed_folders(db: &Database, since: &str) -> Result<Vec<SyncFolder>, SyncError> {
    let rows: Vec<(String, String, Option<String>, Option<String>, i64, String, String)> =
        sqlx::query_as(
            "SELECT uuid, name, icon, color, position, created_at, COALESCE(updated_at, created_at)
             FROM folders WHERE uuid IS NOT NULL AND COALESCE(updated_at, created_at) > ?"
        ).bind(since).fetch_all(&db.pool).await?;

    Ok(rows.into_iter().map(|(uuid, name, icon, color, position, created_at, updated_at)| {
        SyncFolder { uuid, name, icon, color, position, created_at, updated_at }
    }).collect())
}

async fn get_tombstones(db: &Database) -> Result<Vec<Tombstone>, SyncError> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT uuid, entity_type, deleted_at FROM sync_tombstones"
    ).fetch_all(&db.pool).await?;

    Ok(rows.into_iter().map(|(uuid, et, da)| Tombstone {
        uuid, entity_type: et, deleted_at: da,
    }).collect())
}

async fn resolve_folder_uuid(db: &Database, folder_id: Option<i64>) -> Result<Option<String>, SyncError> {
    if let Some(fid) = folder_id {
        Ok(sqlx::query_scalar("SELECT uuid FROM folders WHERE id = ?")
            .bind(fid).fetch_optional(&db.pool).await?.flatten())
    } else {
        Ok(None)
    }
}

// ── Apply delta to local DB ──

pub(crate) async fn apply_delta(
    db: &Database,
    delta: &SyncDelta,
    sync_images: bool,
    drive: &DriveClient,
    report: &mut SyncReport,
) -> Result<(), SyncError> {
    // Build local lookup for conflict resolution
    let local_clips: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT uuid, COALESCE(updated_at, created_at) FROM clips"
    ).fetch_all(&db.pool).await?.into_iter().collect();

    let local_folders: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT uuid, COALESCE(updated_at, created_at) FROM folders WHERE uuid IS NOT NULL"
    ).fetch_all(&db.pool).await?.into_iter().collect();

    // Apply clip changes
    for clip in &delta.clips {
        if clip.clip_type == "image" && !sync_images { continue; }

        let should_apply = match local_clips.get(&clip.uuid) {
            None => true,
            Some(local_ts) => clip.updated_at > *local_ts,
        };
        if !should_apply { continue; }

        let folder_id: Option<i64> = if let Some(ref fuuid) = clip.folder_uuid {
            sqlx::query_scalar("SELECT id FROM folders WHERE uuid = ?")
                .bind(fuuid).fetch_optional(&db.pool).await?
        } else { None };

        let content: Vec<u8> = if clip.clip_type == "image" {
            let img_filename = format!("{}.png", clip.content_hash);
            if sync_images {
                let image_path = db.images_dir.join(&img_filename);
                if !image_path.exists() {
                    let drive_img_name = format!("img_{}.png", clip.content_hash);
                    if let Some(img_file) = drive.find_file_by_name(&drive_img_name).await? {
                        if let Ok(img_bytes) = drive.download_file(&img_file.id).await {
                            if std::fs::write(&image_path, &img_bytes).is_ok() {
                                if let Some(thumb) = crate::clipboard::generate_thumbnail(&img_bytes) {
                                    let thumb_path = db.images_dir.join(format!("{}_thumb.jpg", clip.content_hash));
                                    let _ = std::fs::write(&thumb_path, &thumb);
                                }
                            }
                        }
                    }
                }
            }
            img_filename.into_bytes()
        } else {
            clip.text_content.as_deref().unwrap_or("").as_bytes().to_vec()
        };

        if local_clips.contains_key(&clip.uuid) {
            // UUID match — update in place
            sqlx::query(
                "UPDATE clips SET clip_type=?, content=?, text_preview=?, content_hash=?,
                        folder_id=?, source_app=?, metadata=?, subtype=?, note=?,
                        paste_count=MAX(paste_count,?), is_pinned=?, is_sensitive=?, updated_at=?
                 WHERE uuid=?"
            )
            .bind(&clip.clip_type).bind(&content).bind(&clip.text_preview)
            .bind(&clip.content_hash).bind(folder_id).bind(&clip.source_app)
            .bind(&clip.metadata).bind(&clip.subtype).bind(&clip.note)
            .bind(clip.paste_count).bind(clip.is_pinned).bind(clip.is_sensitive)
            .bind(&clip.updated_at).bind(&clip.uuid)
            .execute(&db.pool).await?;
        } else {
            // UUID not found locally — check if same content already exists (e.g. after DB repair)
            let existing_by_hash: Option<String> = sqlx::query_scalar(
                "SELECT uuid FROM clips WHERE content_hash = ? LIMIT 1"
            ).bind(&clip.content_hash).fetch_optional(&db.pool).await?;

            if let Some(local_uuid) = existing_by_hash {
                // Same content exists with different UUID — adopt remote UUID and update
                sqlx::query(
                    "UPDATE clips SET uuid=?, clip_type=?, text_preview=?, folder_id=?,
                            source_app=?, metadata=?, subtype=?, note=?,
                            paste_count=MAX(paste_count,?), is_pinned=?, is_sensitive=?, updated_at=?
                     WHERE uuid=?"
                )
                .bind(&clip.uuid).bind(&clip.clip_type).bind(&clip.text_preview)
                .bind(folder_id).bind(&clip.source_app).bind(&clip.metadata)
                .bind(&clip.subtype).bind(&clip.note).bind(clip.paste_count)
                .bind(clip.is_pinned).bind(clip.is_sensitive)
                .bind(&clip.updated_at).bind(&local_uuid)
                .execute(&db.pool).await?;
            } else {
                // Truly new clip — insert
                sqlx::query(
                    "INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash,
                            folder_id, is_deleted, source_app, metadata, subtype, note,
                            paste_count, is_pinned, is_sensitive, created_at, last_accessed, updated_at)
                     VALUES (?,?,?,?,?,?,0,?,?,?,?,?,?,?,?,?,?)"
                )
                .bind(&clip.uuid).bind(&clip.clip_type).bind(&content)
                .bind(&clip.text_preview).bind(&clip.content_hash)
                .bind(folder_id).bind(&clip.source_app).bind(&clip.metadata)
                .bind(&clip.subtype).bind(&clip.note).bind(clip.paste_count)
                .bind(clip.is_pinned).bind(clip.is_sensitive)
                .bind(&clip.created_at).bind(&clip.updated_at).bind(&clip.updated_at)
                .execute(&db.pool).await?;
            }
        }
        crate::clipboard::add_to_search_cache(&clip.uuid, &clip.text_preview, folder_id);
        report.pulled_clips += 1;
    }

    // Apply folder changes
    for folder in &delta.folders {
        // Skip folders with "(synced)" suffix — these are artifacts from a previous bug
        if folder.name.ends_with(" (synced)") {
            log::info!("Sync: skipping artifact folder '{}' (uuid={})", folder.name, folder.uuid);
            continue;
        }

        let should_apply = match local_folders.get(&folder.uuid) {
            None => true,
            Some(local_ts) => folder.updated_at > *local_ts,
        };
        if !should_apply { continue; }

        if local_folders.contains_key(&folder.uuid) {
            // UUID match — update in place (keep local name to preserve user's organization)
            sqlx::query("UPDATE folders SET icon=?, color=?, position=?, updated_at=? WHERE uuid=?")
                .bind(&folder.icon).bind(&folder.color)
                .bind(folder.position).bind(&folder.updated_at).bind(&folder.uuid)
                .execute(&db.pool).await?;
        } else {
            // UUID not found locally — check if same-name folder exists (e.g. after DB repair)
            let existing_by_name: Option<i64> = sqlx::query_scalar("SELECT id FROM folders WHERE name = ?")
                .bind(&folder.name).fetch_optional(&db.pool).await?;

            if let Some(local_id) = existing_by_name {
                // Same-name folder exists — adopt remote UUID (reconcile identity)
                log::info!("Sync: reconciling folder '{}' — adopting remote uuid={}", folder.name, folder.uuid);
                sqlx::query("UPDATE folders SET uuid=?, icon=?, color=?, position=?, updated_at=? WHERE id=?")
                    .bind(&folder.uuid).bind(&folder.icon).bind(&folder.color)
                    .bind(folder.position).bind(&folder.updated_at).bind(local_id)
                    .execute(&db.pool).await?;
            } else {
                // Truly new folder — insert
                sqlx::query("INSERT INTO folders (uuid, name, icon, color, position, created_at, updated_at) VALUES (?,?,?,?,?,?,?)")
                    .bind(&folder.uuid).bind(&folder.name).bind(&folder.icon).bind(&folder.color)
                    .bind(folder.position).bind(&folder.created_at).bind(&folder.updated_at)
                    .execute(&db.pool).await?;
            }
        }
        report.pulled_folders += 1;
    }

    // Cleanup: merge any existing "(synced)" folders into their originals
    let synced_folders: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM folders WHERE name LIKE '% (synced)'"
    ).fetch_all(&db.pool).await.unwrap_or_default();
    for (synced_id, synced_name) in &synced_folders {
        let orig_name = synced_name.trim_end_matches(" (synced)");
        let orig_id: Option<i64> = sqlx::query_scalar("SELECT id FROM folders WHERE name = ?")
            .bind(orig_name).fetch_optional(&db.pool).await.unwrap_or(None);
        if let Some(oid) = orig_id {
            let _ = sqlx::query("UPDATE clips SET folder_id = ? WHERE folder_id = ?")
                .bind(oid).bind(synced_id).execute(&db.pool).await;
            let _ = sqlx::query("DELETE FROM folders WHERE id = ?")
                .bind(synced_id).execute(&db.pool).await;
            log::info!("Sync cleanup: merged '{}' into '{}'", synced_name, orig_name);
        } else {
            let _ = sqlx::query("UPDATE folders SET name = ? WHERE id = ?")
                .bind(orig_name).bind(synced_id).execute(&db.pool).await;
            log::info!("Sync cleanup: renamed '{}' → '{}'", synced_name, orig_name);
        }
        report.deleted += 1;
    }

    // Apply tombstones
    for tombstone in &delta.tombstones {
        match tombstone.entity_type.as_str() {
            "clip" => {
                if let Some((clip_type, content)) = sqlx::query_as::<_, (String, Vec<u8>)>(
                    "SELECT clip_type, content FROM clips WHERE uuid=?"
                ).bind(&tombstone.uuid).fetch_optional(&db.pool).await? {
                    if clip_type == "image" {
                        db.remove_image_and_thumb(&String::from_utf8_lossy(&content));
                    }
                    sqlx::query("DELETE FROM clips WHERE uuid=?")
                        .bind(&tombstone.uuid).execute(&db.pool).await?;
                    crate::clipboard::remove_from_search_cache(&tombstone.uuid);
                    report.deleted += 1;
                }
            }
            "folder" => {
                if let Some(fid) = sqlx::query_scalar::<_, i64>("SELECT id FROM folders WHERE uuid=?")
                    .bind(&tombstone.uuid).fetch_optional(&db.pool).await? {
                    // Collect affected clip uuids BEFORE the update so we can patch the cache
                    let affected: Vec<String> = sqlx::query_scalar(
                        "SELECT uuid FROM clips WHERE folder_id=?"
                    ).bind(fid).fetch_all(&db.pool).await.unwrap_or_default();
                    sqlx::query("UPDATE clips SET folder_id=NULL, updated_at=CURRENT_TIMESTAMP WHERE folder_id=?")
                        .bind(fid).execute(&db.pool).await?;
                    sqlx::query("DELETE FROM folders WHERE id=?").bind(fid).execute(&db.pool).await?;
                    // Patch in-memory search cache so clips are no longer associated with the deleted folder
                    {
                        let mut cache = crate::clipboard::SEARCH_CACHE.write();
                        for uuid in &affected {
                            if let Some(entry) = cache.get_mut(uuid) {
                                entry.1 = None;
                            }
                        }
                    }
                    report.deleted += 1;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

// ── Compact: merge all deltas into full state ──

async fn compact(
    db: &Database,
    drive: &DriveClient,
    device_id: &str,
    sync_images: bool,
    delta_files: &[super::drive::DriveFile],
) -> Result<(), SyncError> {
    // Upload fresh full state
    let state = build_full_state(db, device_id, sync_images).await?;
    let json = serde_json::to_vec(&state)?;
    drive.upsert_file(STATE_FILE, &json, "application/json").await?;

    // Delete all delta files
    for file in delta_files {
        if file.name.starts_with(DELTA_PREFIX) {
            drive.delete_file(&file.id).await.ok();
        }
    }

    log::info!("SYNC: Compacted — {} clips, {} folders in state, {} deltas removed",
        state.clips.len(), state.folders.len(), delta_files.iter().filter(|f| f.name.starts_with(DELTA_PREFIX)).count());
    Ok(())
}

// ── Image sync ──

async fn get_remote_image_hashes(drive: &DriveClient) -> Result<std::collections::HashSet<String>, SyncError> {
    let files = drive.list_files(Some("img_"), None).await?;
    Ok(files.iter()
        .filter_map(|f| f.name.strip_prefix("img_").and_then(|n| n.strip_suffix(".png")).map(|s| s.to_string()))
        .collect())
}

async fn push_new_images(
    db: &Database,
    drive: &DriveClient,
    remote_hashes: &std::collections::HashSet<String>,
    report: &mut SyncReport,
) -> Result<(), SyncError> {
    let local_images: Vec<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT content_hash, content FROM clips WHERE clip_type = 'image'"
    ).fetch_all(&db.pool).await?;

    for (hash, content) in &local_images {
        if remote_hashes.contains(hash) { continue; }

        let filename = String::from_utf8_lossy(content).into_owned();
        let path = db.images_dir.join(&filename);
        if !path.exists() { continue; }

        match std::fs::read(&path) {
            Ok(bytes) if bytes.len() <= 10_000_000 => {
                let drive_name = format!("img_{}.png", hash);
                if let Err(e) = drive.upsert_file(&drive_name, &bytes, "image/png").await {
                    log::warn!("SYNC: Failed to push image {}: {}", hash, e);
                    report.errors.push(format!("push image: {}", e));
                }
            }
            Ok(bytes) => log::info!("SYNC: Skipping large image {} ({}MB)", hash, bytes.len() / 1_000_000),
            Err(e) => log::warn!("SYNC: Failed to read image {}: {}", filename, e),
        }
    }
    Ok(())
}

// ── Helpers ──

async fn save_setting(pool: &sqlx::SqlitePool, key: &str, value: &str) -> Result<(), SyncError> {
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
        .bind(key).bind(value).execute(pool).await?;
    Ok(())
}
