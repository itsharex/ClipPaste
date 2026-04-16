use super::drive::DriveClient;
use super::error::SyncError;
use super::models::*;
use crate::database::Database;

const STATE_FILE: &str = "sync_state.json";
/// Legacy per-device delta file prefix (v1.8.6 and earlier). Kept for backwards-compat:
/// on upgrade we still pull any leftover `delta_*.json` so no data is lost, and compact
/// deletes them. New writes always use `OP_PREFIX`.
const LEGACY_DELTA_PREFIX: &str = "delta_";
/// Append-only op file prefix. Filenames: `op_{device_id}_{millis}.json`.
const OP_PREFIX: &str = "op_";
/// Compact when accumulated op files exceed this count…
const MAX_OPS_BEFORE_COMPACT: usize = 30;
/// …or when their total size exceeds this many bytes.
const MAX_OPS_BYTES_BEFORE_COMPACT: u64 = 2 * 1024 * 1024;

/// Full snapshot — uploaded once, then compacted periodically.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct SyncState {
    pub(crate) clips: Vec<SyncClip>,
    pub(crate) folders: Vec<SyncFolder>,
    #[serde(default)]
    pub(crate) scratchpads: Vec<SyncScratchpad>,
    pub(crate) tombstones: Vec<Tombstone>,
    pub(crate) device_id: String,
    pub(crate) updated_at: String,
}

/// Small delta — only contains changes since last sync.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SyncDelta {
    pub(crate) clips: Vec<SyncClip>,
    pub(crate) folders: Vec<SyncFolder>,
    #[serde(default)]
    pub(crate) scratchpads: Vec<SyncScratchpad>,
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

/// Run a sync cycle using append-only op log + periodic compaction.
///
/// Fixes a data-loss bug in v1.8.6 and earlier where every push overwrote the
/// same `delta_{device_id}.json` file, so any clips pushed between two consecutive
/// syncs by the same device would vanish if the peer had not pulled in between.
///
/// New model:
///   - Each push creates a uniquely-named `op_{device}_{millis}.json` (never overwrites).
///   - `sync_push_base_at` (RFC3339) is the "everything ≤ this has already been pushed"
///     watermark, advanced at the end of each successful sync. Decoupled from the
///     pull watermark so re-pushes after the last advance are safe.
///   - `sync_ops_cursor_ms` is the millis of the newest op we've applied; we only
///     download+apply ops whose filename-millis is strictly greater.
///   - `sync_state_seen_modified` tracks the last `modifiedTime` of `sync_state.json`
///     we've reconciled against, so a remote compaction re-seeds every peer.
///   - Compaction: when op files >= MAX_OPS_BEFORE_COMPACT or total size exceeds
///     MAX_OPS_BYTES_BEFORE_COMPACT, rebuild full state and delete all op files
///     (both new `op_*` and legacy `delta_*`).
pub async fn sync_now(
    db: &Database,
    drive: &DriveClient,
    sync_images: bool,
) -> Result<SyncReport, SyncError> {
    let mut report = SyncReport::default();

    let device_id = super::get_device_id(db).await
        .unwrap_or_else(|| "unknown".to_string());

    // Captured once at the start: becomes the new push_base if the cycle succeeds.
    let cycle_started_at = chrono::Utc::now();
    let cycle_started_iso = cycle_started_at.to_rfc3339();

    // 1. First-sync path — no state file on Drive yet.
    let state_meta = drive.find_file_by_name(STATE_FILE).await?;
    if state_meta.is_none() {
        log::info!("SYNC: First sync — uploading full state");
        let state = build_full_state(db, &device_id, sync_images).await?;
        let json = serde_json::to_vec(&state)?;
        let uploaded = drive.upsert_file(STATE_FILE, &json, "application/json").await?;
        report.pushed_clips = state.clips.len() as u64;
        report.pushed_folders = state.folders.len() as u64;

        if sync_images {
            push_new_images(db, drive, &std::collections::HashSet::new(), &mut report).await?;
        }

        save_setting(&db.pool, "sync_push_base_at", &cycle_started_iso).await?;
        save_setting(&db.pool, "sync_ops_cursor_ms", &cycle_started_at.timestamp_millis().to_string()).await?;
        if let Some(modified) = uploaded.modified_time.as_deref() {
            save_setting(&db.pool, "sync_state_seen_modified", modified).await?;
        }
        save_setting(&db.pool, "sync_last_sync_at", &cycle_started_iso).await?;
        log::info!("SYNC: First sync complete — {} clips, {} folders", report.pushed_clips, report.pushed_folders);
        return Ok(report);
    }
    let state_meta = state_meta.unwrap();

    // Load local cursors (with defaults for upgraded installs).
    let push_base_at = db.get_setting("sync_push_base_at").await
        .map_err(|e| SyncError::Database(e.to_string()))?
        .or_else(|| {
            // Migration: fall back to legacy `sync_last_sync_at` so we don't re-push everything.
            // On first sync with the new code, this just means we treat the last successful
            // legacy sync as the push watermark.
            None
        })
        .or(db.get_setting("sync_last_sync_at").await.ok().flatten())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

    let ops_cursor_ms: i64 = db.get_setting("sync_ops_cursor_ms").await
        .map_err(|e| SyncError::Database(e.to_string()))?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let state_seen_modified = db.get_setting("sync_state_seen_modified").await
        .map_err(|e| SyncError::Database(e.to_string()))?;

    // 2. Reconcile against sync_state.json if a peer compacted since we last looked.
    let mut next_ops_cursor_ms = ops_cursor_ms;
    let state_refreshed = state_meta.modified_time.as_deref() != state_seen_modified.as_deref();
    if state_refreshed {
        log::info!("SYNC: sync_state.json changed on Drive — reconciling full state");
        let data = drive.download_file(&state_meta.id).await?;
        match serde_json::from_slice::<SyncState>(&data) {
            Ok(state) => {
                let state_as_delta = SyncDelta {
                    clips: state.clips,
                    folders: state.folders,
                    scratchpads: state.scratchpads,
                    tombstones: state.tombstones,
                    device_id: state.device_id,
                    created_at: state.updated_at.clone(),
                };
                apply_delta(db, &state_as_delta, sync_images, drive, &mut report).await?;
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&state.updated_at) {
                    next_ops_cursor_ms = next_ops_cursor_ms.max(dt.timestamp_millis());
                }
                if let Some(modified) = state_meta.modified_time.as_deref() {
                    save_setting(&db.pool, "sync_state_seen_modified", modified).await?;
                }
            }
            Err(e) => log::warn!("SYNC: Failed to parse sync_state.json: {}", e),
        }
    }

    // 3. Pull: list all op files (new `op_*` + legacy `delta_*`) and apply any we haven't seen.
    let op_files = drive.list_files(Some(OP_PREFIX), None).await?;
    let legacy_files = drive.list_files(Some(LEGACY_DELTA_PREFIX), None).await?;

    let own_op_prefix = format!("{}{}_", OP_PREFIX, device_id);
    let own_legacy_name = format!("{}{}.json", LEGACY_DELTA_PREFIX, device_id);

    // Apply new-style ops in chronological order, skipping our own and anything ≤ cursor.
    let mut sorted_ops: Vec<(&super::drive::DriveFile, i64)> = op_files.iter()
        .filter(|f| f.name.starts_with(OP_PREFIX) && !f.name.starts_with(&own_op_prefix))
        .filter_map(|f| parse_op_millis(&f.name).map(|ms| (f, ms)))
        .filter(|(_, ms)| *ms > ops_cursor_ms)
        .collect();
    sorted_ops.sort_by_key(|(_, ms)| *ms);

    for (op_file, ms) in &sorted_ops {
        let data = drive.download_file(&op_file.id).await?;
        match serde_json::from_slice::<SyncDelta>(&data) {
            Ok(delta) if delta.device_id != device_id => {
                apply_delta(db, &delta, sync_images, drive, &mut report).await?;
                next_ops_cursor_ms = next_ops_cursor_ms.max(*ms);
            }
            Ok(_) => {} // our own op (shouldn't happen after prefix filter, but safe)
            Err(e) => log::warn!("SYNC: Failed to parse op {}: {}", op_file.name, e),
        }
    }

    // Apply legacy delta files once (idempotent via LWW). They'll be deleted at next compact.
    for legacy in legacy_files.iter()
        .filter(|f| f.name.starts_with(LEGACY_DELTA_PREFIX) && f.name != own_legacy_name)
    {
        let data = drive.download_file(&legacy.id).await?;
        match serde_json::from_slice::<SyncDelta>(&data) {
            Ok(delta) if delta.device_id != device_id => {
                log::info!("SYNC: Applying legacy delta {}", legacy.name);
                apply_delta(db, &delta, sync_images, drive, &mut report).await?;
            }
            Ok(_) => {}
            Err(e) => log::warn!("SYNC: Failed to parse legacy {}: {}", legacy.name, e),
        }
    }

    // 4. Collect local changes since push_base_at and push as a NEW op file (never overwrite).
    let changed_clips = get_changed_clips(db, &push_base_at, sync_images).await?;
    let changed_folders = get_changed_folders(db, &push_base_at).await?;
    let changed_scratchpads = get_changed_scratchpads(db, &push_base_at).await?;
    let tombstones = get_tombstones(db).await?;

    let has_local_changes = !changed_clips.is_empty() || !changed_folders.is_empty() || !changed_scratchpads.is_empty() || !tombstones.is_empty();

    if has_local_changes {
        let delta = SyncDelta {
            clips: changed_clips,
            folders: changed_folders,
            scratchpads: changed_scratchpads,
            tombstones,
            device_id: device_id.clone(),
            created_at: cycle_started_iso.clone(),
        };
        report.pushed_clips = delta.clips.len() as u64;
        report.pushed_folders = delta.folders.len() as u64;

        let op_name = format!("{}{}_{}.json", OP_PREFIX, device_id, cycle_started_at.timestamp_millis());
        let json = serde_json::to_vec(&delta)?;
        log::info!("SYNC: Uploading op {} ({} bytes, {} clips, {} folders)",
            op_name, json.len(), delta.clips.len(), delta.folders.len());
        drive.create_file(&op_name, &json, "application/json").await?;

        if sync_images {
            let remote_hashes = get_remote_image_hashes(drive).await?;
            push_new_images(db, drive, &remote_hashes, &mut report).await?;
        }
    } else if sorted_ops.is_empty() && !state_refreshed {
        log::info!("SYNC: No changes anywhere, skipping");
        report.skipped = true;
    }

    // 5. Compact if the op log is getting large. Includes legacy files in the size/count check.
    let total_op_count = op_files.iter().filter(|f| f.name.starts_with(OP_PREFIX)).count()
        + legacy_files.iter().filter(|f| f.name.starts_with(LEGACY_DELTA_PREFIX)).count();
    let total_op_bytes: u64 = op_files.iter().chain(legacy_files.iter())
        .filter(|f| f.name.starts_with(OP_PREFIX) || f.name.starts_with(LEGACY_DELTA_PREFIX))
        .filter_map(|f| f.size.as_deref().and_then(|s| s.parse::<u64>().ok()))
        .sum();

    if total_op_count >= MAX_OPS_BEFORE_COMPACT || total_op_bytes >= MAX_OPS_BYTES_BEFORE_COMPACT {
        log::info!("SYNC: Compacting {} op files ({} bytes) into fresh state", total_op_count, total_op_bytes);
        let merged_files: Vec<super::drive::DriveFile> = op_files.iter().chain(legacy_files.iter()).cloned().collect();
        if let Some(new_state_meta) = compact(db, drive, &device_id, sync_images, &merged_files).await? {
            if let Some(modified) = new_state_meta.modified_time.as_deref() {
                save_setting(&db.pool, "sync_state_seen_modified", modified).await?;
            }
            // After compact, everything up to now is captured in the state snapshot.
            next_ops_cursor_ms = cycle_started_at.timestamp_millis();
        }
    }

    // 6. Advance watermarks and save.
    save_setting(&db.pool, "sync_push_base_at", &cycle_started_iso).await?;
    save_setting(&db.pool, "sync_ops_cursor_ms", &next_ops_cursor_ms.to_string()).await?;
    save_setting(&db.pool, "sync_last_sync_at", &cycle_started_iso).await?;
    super::cleanup_tombstones(db).await.ok();

    log::info!("SYNC: Complete — pushed {}/{}, pulled {}/{}, deleted {}",
        report.pushed_clips, report.pushed_folders, report.pulled_clips, report.pulled_folders, report.deleted);

    Ok(report)
}

/// Parse the unix millis from an op filename `op_{device_id}_{millis}.json`.
/// Returns None if the format doesn't match (e.g. legacy `delta_*` files).
fn parse_op_millis(name: &str) -> Option<i64> {
    let stem = name.strip_prefix(OP_PREFIX)?.strip_suffix(".json")?;
    let millis_str = stem.rsplit_once('_')?.1;
    millis_str.parse::<i64>().ok()
}

// ── Build state from DB ──

pub(crate) async fn build_full_state(db: &Database, device_id: &str, sync_images: bool) -> Result<SyncState, SyncError> {
    let clips = get_all_clips(db, sync_images).await?;
    let folders = get_all_folders(db).await?;
    let scratchpads = get_all_scratchpads(db).await?;
    let tombstones = get_tombstones(db).await?;

    Ok(SyncState {
        clips,
        folders,
        scratchpads,
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

async fn get_all_scratchpads(db: &Database) -> Result<Vec<SyncScratchpad>, SyncError> {
    let rows: Vec<(String, String, String, Option<String>, bool, Option<String>, i64, String, String)> = sqlx::query_as(
        "SELECT uuid, title, content, fields_json, is_pinned, color, position, created_at, COALESCE(updated_at, created_at)
         FROM scratchpads ORDER BY position ASC"
    ).fetch_all(&db.pool).await?;

    Ok(rows.into_iter().map(|(uuid, title, content, fields_json, is_pinned, color, position, created_at, updated_at)| {
        SyncScratchpad { uuid, title, content, fields_json, is_pinned, color, position, created_at, updated_at }
    }).collect())
}

async fn get_changed_scratchpads(db: &Database, since: &str) -> Result<Vec<SyncScratchpad>, SyncError> {
    let rows: Vec<(String, String, String, Option<String>, bool, Option<String>, i64, String, String)> = sqlx::query_as(
        "SELECT uuid, title, content, fields_json, is_pinned, color, position, created_at, COALESCE(updated_at, created_at)
         FROM scratchpads WHERE COALESCE(updated_at, created_at) > ?"
    ).bind(since).fetch_all(&db.pool).await?;

    Ok(rows.into_iter().map(|(uuid, title, content, fields_json, is_pinned, color, position, created_at, updated_at)| {
        SyncScratchpad { uuid, title, content, fields_json, is_pinned, color, position, created_at, updated_at }
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

    // Apply scratchpad changes
    let local_scratchpads: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT uuid, COALESCE(updated_at, created_at) FROM scratchpads"
    ).fetch_all(&db.pool).await.unwrap_or_default().into_iter().collect();

    for sp in &delta.scratchpads {
        let should_apply = match local_scratchpads.get(&sp.uuid) {
            None => true,
            Some(local_ts) => sp.updated_at > *local_ts,
        };
        if !should_apply { continue; }

        if local_scratchpads.contains_key(&sp.uuid) {
            sqlx::query("UPDATE scratchpads SET title=?, content=?, fields_json=?, is_pinned=?, color=?, position=?, updated_at=? WHERE uuid=?")
                .bind(&sp.title).bind(&sp.content).bind(&sp.fields_json).bind(sp.is_pinned).bind(&sp.color)
                .bind(sp.position).bind(&sp.updated_at).bind(&sp.uuid)
                .execute(&db.pool).await?;
        } else {
            sqlx::query("INSERT INTO scratchpads (uuid, title, content, fields_json, is_pinned, color, position, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?)")
                .bind(&sp.uuid).bind(&sp.title).bind(&sp.content).bind(&sp.fields_json).bind(sp.is_pinned).bind(&sp.color)
                .bind(sp.position).bind(&sp.created_at).bind(&sp.updated_at)
                .execute(&db.pool).await?;
        }
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
            "scratchpad" => {
                let deleted = sqlx::query("DELETE FROM scratchpads WHERE uuid=?")
                    .bind(&tombstone.uuid).execute(&db.pool).await?;
                if deleted.rows_affected() > 0 {
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
    op_files: &[super::drive::DriveFile],
) -> Result<Option<super::drive::DriveFile>, SyncError> {
    // Upload fresh full state
    let state = build_full_state(db, device_id, sync_images).await?;
    let json = serde_json::to_vec(&state)?;
    let new_state_meta = drive.upsert_file(STATE_FILE, &json, "application/json").await?;

    // Delete all op log files (both new-style and legacy)
    let mut removed = 0usize;
    for file in op_files {
        if file.name.starts_with(OP_PREFIX) || file.name.starts_with(LEGACY_DELTA_PREFIX) {
            if drive.delete_file(&file.id).await.is_ok() {
                removed += 1;
            }
        }
    }

    log::info!("SYNC: Compacted — {} clips, {} folders in state, {} op files removed",
        state.clips.len(), state.folders.len(), removed);
    Ok(Some(new_state_meta))
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
