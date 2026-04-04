use tauri::{AppHandle, Emitter};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use crate::database::Database;
use crate::models::{Clip, ClipboardItem};
use super::helpers::{clip_to_item_async, check_auto_paste_and_hide, clipboard_write_text, clipboard_set_hashes};

/// Simple fuzzy match: checks if all characters of `needle` appear in `haystack` in order.
/// E.g. "apikey" fuzzy-matches "api_key", "API_KEY", "my_api_key_value".
pub fn fuzzy_contains(haystack: &str, needle: &str) -> bool {
    let mut hay_chars = haystack.chars();
    for nc in needle.chars() {
        loop {
            match hay_chars.next() {
                Some(hc) if hc == nc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

#[tauri::command]
pub async fn get_clips(filter_id: Option<String>, limit: i64, offset: i64, preview_only: Option<bool>, db: tauri::State<'_, Arc<Database>>) -> Result<Vec<ClipboardItem>, String> {
    let pool = &db.pool;
    let preview_only = preview_only.unwrap_or(false);

    log::debug!("get_clips called with filter_id: {:?}, preview_only: {}", filter_id, preview_only);

    let clips: Vec<Clip> = match filter_id.as_deref() {
        Some("__frequent__") => {
            log::debug!("Querying for frequently pasted clips");
            sqlx::query_as(r#"
                SELECT id, uuid, clip_type,
                       CASE WHEN clip_type = 'image' THEN content ELSE '' END as content,
                       text_preview, content_hash,
                       folder_id, is_deleted, source_app, source_icon, metadata,
                       created_at, last_accessed, last_pasted_at, is_pinned,
                       subtype, note, paste_count, is_sensitive
                FROM clips WHERE paste_count >= 5
                ORDER BY paste_count DESC, created_at DESC
                LIMIT ? OFFSET ?
            "#)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool).await.map_err(|e| e.to_string())?
        }
        Some(id) => {
            let folder_id_num = id.parse::<i64>().ok();
            if let Some(numeric_id) = folder_id_num {
                log::debug!("Querying for folder_id: {}", numeric_id);
                sqlx::query_as(r#"
                    SELECT id, uuid, clip_type,
                           CASE WHEN clip_type = 'image' THEN content ELSE '' END as content,
                           text_preview, content_hash,
                           folder_id, is_deleted, source_app, source_icon, metadata,
                           created_at, last_accessed, last_pasted_at, is_pinned,
                           subtype, note, paste_count, is_sensitive
                    FROM clips WHERE folder_id = ?
                    ORDER BY is_pinned DESC,
                             CASE WHEN note IS NOT NULL AND note != '' THEN 0 ELSE 1 END,
                             CASE WHEN note IS NOT NULL AND note != '' THEN note ELSE NULL END,
                             created_at DESC
                    LIMIT ? OFFSET ?
                "#)
                .bind(numeric_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool).await.map_err(|e| e.to_string())?
            } else {
                log::debug!("Unknown folder_id, returning empty");
                Vec::new()
            }
        }
        None => {
            log::debug!("Querying for items, offset: {}, limit: {}", offset, limit);
            sqlx::query_as(r#"
                SELECT id, uuid, clip_type,
                       CASE WHEN clip_type = 'image' THEN content ELSE '' END as content,
                       text_preview, content_hash,
                       folder_id, is_deleted, source_app, source_icon, metadata,
                       created_at, last_accessed, last_pasted_at, is_pinned,
                       subtype, note, paste_count, is_sensitive
                FROM clips
                ORDER BY created_at DESC LIMIT ? OFFSET ?
            "#)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool).await.map_err(|e| e.to_string())?
        }
    };

    log::debug!("DB: Found {} clips", clips.len());

    let mut items = Vec::with_capacity(clips.len());
    for (idx, clip) in clips.iter().enumerate() {
        if idx < 10 {
            let content_len = if clip.clip_type == "image" {
                if preview_only { 0 } else { clip.content.len() }
            } else {
                clip.text_preview.len()
            };
            log::trace!("{} Clip {}: type='{}', content_len={}", idx, clip.uuid, clip.clip_type, content_len);
        }
        items.push(clip_to_item_async(clip, &db.images_dir, preview_only).await);
    }

    Ok(items)
}

#[tauri::command]
pub async fn get_clip(id: String, db: tauri::State<'_, Arc<Database>>) -> Result<ClipboardItem, String> {
    let pool = &db.pool;

    let clip: Option<Clip> = sqlx::query_as(r#"SELECT * FROM clips WHERE uuid = ?"#)
        .bind(&id)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            let content_str = if clip.clip_type == "image" {
                use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
                let filename = String::from_utf8_lossy(&clip.content).to_string();
                let image_path = db.images_dir.join(&filename);
                match std::fs::read(&image_path) {
                    Ok(bytes) => BASE64.encode(&bytes),
                    Err(_) => String::new(),
                }
            } else {
                String::from_utf8_lossy(&clip.content).to_string()
            };

            Ok(ClipboardItem {
                id: clip.uuid,
                clip_type: clip.clip_type,
                content: content_str,
                preview: clip.text_preview,
                folder_id: clip.folder_id.map(|id| id.to_string()),
                created_at: clip.created_at.to_rfc3339(),
                source_app: clip.source_app,
                source_icon: clip.source_icon,
                metadata: clip.metadata,
                is_pinned: clip.is_pinned,
                subtype: clip.subtype,
                note: clip.note,
                paste_count: clip.paste_count,
                is_sensitive: clip.is_sensitive,
            })
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn paste_clip(id: String, app: AppHandle, window: tauri::WebviewWindow, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    // Only fetch columns needed for paste (skip source_icon, metadata, etc.)
    let clip: Option<Clip> = sqlx::query_as(
        "SELECT id, uuid, clip_type, content, text_preview, content_hash,
                folder_id, is_deleted, source_app, '' as source_icon, metadata,
                created_at, last_accessed, last_pasted_at, is_pinned,
                subtype, note, paste_count, is_sensitive
         FROM clips WHERE uuid = ?"
    )
        .bind(&id)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            let content_hash = clip.content_hash.clone();
            let uuid = clip.uuid.clone();

            let final_res = if clip.clip_type == "image" {
                clipboard_set_hashes(&content_hash).await;
                log::debug!("Frontend handled image. Skipping backend write.");
                Ok(())
            } else {
                let content_str = String::from_utf8_lossy(&clip.content).to_string();
                clipboard_write_text(&app, &content_str, &content_hash).await
            };

            // Track when this clip was last pasted + increment paste count
            let _ = sqlx::query(r#"UPDATE clips SET last_pasted_at = CURRENT_TIMESTAMP, paste_count = paste_count + 1 WHERE uuid = ?"#)
                .bind(&uuid)
                .execute(pool)
                .await;

            if final_res.is_ok() {
                let content = if clip.clip_type == "image" { "[Image]".to_string() } else { String::from_utf8_lossy(&clip.content).to_string() };
                let _ = window.emit("clipboard-write", &content);
                check_auto_paste_and_hide(&window);
            }
            final_res
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn copy_clip(id: String, app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    // Only fetch columns needed for copy (skip source_icon, metadata, etc.)
    let clip: Option<Clip> = sqlx::query_as(
        "SELECT id, uuid, clip_type, content, text_preview, content_hash,
                folder_id, is_deleted, source_app, '' as source_icon, metadata,
                created_at, last_accessed, last_pasted_at, is_pinned,
                subtype, note, paste_count, is_sensitive
         FROM clips WHERE uuid = ?"
    )
        .bind(&id)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            let content_hash = clip.content_hash.clone();

            if clip.clip_type != "image" {
                let content_str = String::from_utf8_lossy(&clip.content).to_string();
                clipboard_write_text(&app, &content_str, &content_hash).await?;
            } else {
                clipboard_set_hashes(&content_hash).await;
            }

            // Does NOT hide window or simulate paste
            Ok(())
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn paste_text(content: String, app: AppHandle, window: tauri::WebviewWindow, _db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    clipboard_write_text(&app, &content, &content_hash).await?;

    let _ = window.emit("clipboard-write", &content);
    check_auto_paste_and_hide(&window);

    Ok(())
}

#[tauri::command]
pub async fn delete_clip(id: String, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    // Always clean up image file from disk when deleting
    let clip_info: Option<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT clip_type, content FROM clips WHERE uuid = ?"
    ).bind(&id).fetch_optional(pool).await.map_err(|e| e.to_string())?;

    if let Some((clip_type, content)) = &clip_info {
        if clip_type == "image" {
            let filename = String::from_utf8_lossy(content).to_string();
            let image_path = db.images_dir.join(&filename);
            if image_path.exists() {
                let _ = std::fs::remove_file(&image_path);
            }
        }
    }

    sqlx::query("DELETE FROM clips WHERE uuid = ?")
        .bind(&id)
        .execute(pool).await.map_err(|e| e.to_string())?;

    // Remove from in-memory search cache
    crate::clipboard::remove_from_search_cache(&id);

    Ok(())
}

#[tauri::command]
pub async fn search_clips(query: String, filter_id: Option<String>, limit: i64, _offset: i64, db: tauri::State<'_, Arc<Database>>) -> Result<Vec<ClipboardItem>, String> {
    let pool = &db.pool;

    let query_lower = query.to_lowercase();
    let folder_filter: Option<i64> = filter_id.as_deref()
        .and_then(|id| id.parse::<i64>().ok());

    // Split query into words for multi-word matching
    let query_words: Vec<String> = query_lower.split_whitespace()
        .map(|w| w.to_string())
        .collect();

    // Search ALL clips (cross-folder), match against preview AND note
    // Track match quality: exact phrase > all words > fuzzy
    // Uses HashMap-based SEARCH_CACHE: uuid → (preview, folder_id, note)
    // match_tier: 0=exact phrase, 1=all words substring, 2=note match, 3=fuzzy
    let matched: Vec<(String, Option<i64>, u8)> = {
        let cache = crate::clipboard::SEARCH_CACHE.read();
        cache.iter()
            .filter_map(|(uuid, (preview, fid, note))| {
                // Tier 0: exact phrase match (all words adjacent in original order)
                let exact_phrase = preview.contains(&query_lower);
                if exact_phrase {
                    return Some((uuid.clone(), *fid, 0u8));
                }
                // Tier 1: all words present as substrings (AND match)
                let in_preview = query_words.iter().all(|word| preview.contains(word));
                if in_preview {
                    return Some((uuid.clone(), *fid, 1u8));
                }
                // Tier 2: match in note
                let in_note = !note.is_empty() && query_words.iter().all(|word| note.contains(word));
                if in_note {
                    return Some((uuid.clone(), *fid, 2u8));
                }
                // Tier 3: fuzzy subsequence match
                let fuzzy = query_words.iter().all(|word| fuzzy_contains(preview, word));
                if fuzzy {
                    return Some((uuid.clone(), *fid, 3u8));
                }
                None
            })
            .collect()
    };

    // Sort: relevance FIRST (exact > words > note > fuzzy), folder as tiebreaker
    let mut matched = matched;
    matched.sort_by_key(|(_, fid, tier)| {
        let folder_rank = if let Some(target_fid) = folder_filter {
            if *fid == Some(target_fid) { 0u8 } else if fid.is_some() { 1 } else { 2 }
        } else {
            if fid.is_some() { 0 } else { 1 }
        };
        // Primary: match tier (0=best), Secondary: folder rank
        (*tier, folder_rank)
    });
    let matched: Vec<String> = matched.into_iter()
        .take(limit as usize)
        .map(|(uuid, _, _)| uuid)
        .collect();

    let mut clips: Vec<Clip> = if matched.is_empty() {
        Vec::new()
    } else {
        let placeholders: String = matched.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, uuid, clip_type, '' as content, text_preview, content_hash,
                    folder_id, is_deleted, source_app, source_icon, metadata,
                    created_at, last_accessed, last_pasted_at, is_pinned,
                    subtype, note, paste_count, is_sensitive
             FROM clips WHERE uuid IN ({})",
            placeholders
        );
        let mut q = sqlx::query_as::<_, Clip>(&sql);
        for uuid in &matched {
            q = q.bind(uuid);
        }
        q.fetch_all(pool).await.map_err(|e| e.to_string())?
    };

    // Sort by relevance FIRST, then folder as tiebreaker, then recency
    clips.sort_by(|a, b| {
        let a_preview = a.text_preview.to_lowercase();
        let b_preview = b.text_preview.to_lowercase();

        // 1. Relevance tier: exact phrase > starts_with > all words > rest
        let relevance_tier = |preview: &str| -> u8 {
            if preview.contains(&query_lower) { 0 }              // exact phrase
            else if preview.starts_with(&query_lower) { 0 }      // starts with full query
            else if query_words.iter().all(|w| preview.contains(w)) { 1 } // all words present
            else { 2 }                                            // fuzzy/note only
        };
        let a_rel = relevance_tier(&a_preview);
        let b_rel = relevance_tier(&b_preview);

        // 2. Within same relevance: starts_with bonus
        let a_starts = a_preview.starts_with(&query_lower);
        let b_starts = b_preview.starts_with(&query_lower);

        // 3. Folder as tiebreaker (not primary)
        let folder_rank = |clip: &Clip| -> u8 {
            if let Some(target_fid) = folder_filter {
                if clip.folder_id == Some(target_fid) { 0 } else if clip.folder_id.is_some() { 1 } else { 2 }
            } else {
                if clip.folder_id.is_some() { 0 } else { 1 }
            }
        };

        a_rel.cmp(&b_rel)                          // relevance first
            .then(b_starts.cmp(&a_starts))          // starts_with bonus
            .then(folder_rank(a).cmp(&folder_rank(b))) // folder tiebreaker
            .then(b.created_at.cmp(&a.created_at))  // newest first
    });

    // Search results use text_preview instead of full content for speed.
    // Cards only display ~300 chars anyway. Full content loaded on paste.
    let mut items = Vec::with_capacity(clips.len());
    for clip in &clips {
        items.push(clip_to_item_async(clip, &db.images_dir, false).await);
    }

    Ok(items)
}

#[tauri::command]
pub async fn get_initial_state(
    _filter_id: Option<String>,
    limit: i64,
    db: tauri::State<'_, Arc<Database>>,
) -> Result<serde_json::Value, String> {
    // Batch: fetch clips + folders + total count in parallel
    let pool = &db.pool;
    let images_dir = &db.images_dir;

    let clips_future = async {
        let clips: Vec<Clip> = sqlx::query_as(r#"
            SELECT id, uuid, clip_type,
                   CASE WHEN clip_type = 'image' THEN content ELSE '' END as content,
                   text_preview, content_hash,
                   folder_id, is_deleted, source_app, source_icon, metadata,
                   created_at, last_accessed, last_pasted_at, is_pinned,
                   subtype, note, paste_count, is_sensitive
            FROM clips
            ORDER BY created_at DESC LIMIT ? OFFSET 0
        "#).bind(limit).fetch_all(pool).await.unwrap_or_default();

        let mut items = Vec::with_capacity(clips.len());
        for clip in &clips {
            items.push(clip_to_item_async(clip, images_dir, false).await);
        }
        items
    };

    let folders_future = async {
        let folders: Vec<crate::models::Folder> = sqlx::query_as(r#"SELECT * FROM folders ORDER BY position, id"#)
            .fetch_all(pool).await.unwrap_or_default();
        let counts: Vec<(i64, i64)> = sqlx::query_as(r#"
            SELECT folder_id, COUNT(*) as count FROM clips WHERE folder_id IS NOT NULL GROUP BY folder_id
        "#).fetch_all(pool).await.unwrap_or_default();
        let count_map: std::collections::HashMap<i64, i64> = counts.into_iter().collect();
        folders.iter().map(|f| serde_json::json!({
            "id": f.id.to_string(),
            "name": f.name,
            "icon": f.icon,
            "color": f.color,
            "is_system": f.is_system,
            "item_count": count_map.get(&f.id).unwrap_or(&0),
        })).collect::<Vec<_>>()
    };

    let total_future = async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM clips")
            .fetch_one(pool).await.unwrap_or(0)
    };

    let (clips, folders, total) = tokio::join!(clips_future, folders_future, total_future);

    Ok(serde_json::json!({
        "clips": clips,
        "folders": folders,
        "total_count": total,
    }))
}

#[tauri::command]
pub async fn bulk_delete_clips(ids: Vec<String>, db: tauri::State<'_, Arc<Database>>) -> Result<i64, String> {
    let pool = &db.pool;

    if ids.is_empty() { return Ok(0); }

    // Collect image filenames before deleting
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT clip_type, content FROM clips WHERE uuid IN ({}) AND clip_type = 'image'",
        placeholders
    );
    let mut q = sqlx::query_as::<_, (String, Vec<u8>)>(&sql);
    for id in &ids { q = q.bind(id); }
    let image_clips: Vec<(String, Vec<u8>)> = q.fetch_all(pool).await.unwrap_or_default();

    // Delete all clips
    let del_sql = format!("DELETE FROM clips WHERE uuid IN ({})", placeholders);
    let mut dq = sqlx::query(&del_sql);
    for id in &ids { dq = dq.bind(id); }
    let result = dq.execute(pool).await.map_err(|e| e.to_string())?;

    // Clean up image files
    for (_, content) in &image_clips {
        let filename = String::from_utf8_lossy(content).to_string();
        let image_path = db.images_dir.join(&filename);
        if image_path.exists() { let _ = std::fs::remove_file(&image_path); }
    }

    // Remove from search cache
    for id in &ids {
        crate::clipboard::remove_from_search_cache(id);
    }

    Ok(result.rows_affected() as i64)
}

#[tauri::command]
pub async fn bulk_move_clips(ids: Vec<String>, folder_id: Option<String>, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    if ids.is_empty() { return Ok(()); }

    let folder_id_num = match folder_id {
        Some(id) => Some(id.parse::<i64>().map_err(|_| "Invalid folder ID")?),
        None => None,
    };

    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("UPDATE clips SET folder_id = ? WHERE uuid IN ({})", placeholders);
    let mut q = sqlx::query(&sql).bind(folder_id_num);
    for id in &ids { q = q.bind(id); }
    q.execute(pool).await.map_err(|e| e.to_string())?;

    // Update search cache (HashMap: uuid → (preview, folder_id, note))
    {
        let mut cache = crate::clipboard::SEARCH_CACHE.write();
        for id in &ids {
            if let Some(entry) = cache.get_mut(id) {
                entry.1 = folder_id_num;
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn toggle_pin(id: String, db: tauri::State<'_, Arc<Database>>) -> Result<bool, String> {
    let pool = &db.pool;
    sqlx::query("UPDATE clips SET is_pinned = CASE WHEN is_pinned = 0 THEN 1 ELSE 0 END WHERE uuid = ?")
        .bind(&id)
        .execute(pool).await.map_err(|e| e.to_string())?;

    let is_pinned: bool = sqlx::query_scalar("SELECT is_pinned FROM clips WHERE uuid = ?")
        .bind(&id)
        .fetch_one(pool).await.map_err(|e| e.to_string())?;

    Ok(is_pinned)
}

#[tauri::command]
pub async fn update_note(id: String, note: Option<String>, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;
    sqlx::query("UPDATE clips SET note = ? WHERE uuid = ?")
        .bind(&note)
        .bind(&id)
        .execute(pool).await.map_err(|e| e.to_string())?;

    // Update search cache with new note
    crate::clipboard::update_note_in_search_cache(&id, note.as_deref());

    Ok(())
}
