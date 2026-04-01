use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_x::{write_text, stop_listening, start_listening};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::database::Database;
use crate::models::{Clip, ClipboardItem};
use super::helpers::{clip_to_item_async, check_auto_paste_and_hide};

#[tauri::command]
pub async fn get_clips(filter_id: Option<String>, limit: i64, offset: i64, preview_only: Option<bool>, db: tauri::State<'_, Arc<Database>>) -> Result<Vec<ClipboardItem>, String> {
    let pool = &db.pool;
    let preview_only = preview_only.unwrap_or(false);

    log::info!("get_clips called with filter_id: {:?}, preview_only: {}", filter_id, preview_only);

    let clips: Vec<Clip> = match filter_id.as_deref() {
        Some(id) => {
            let folder_id_num = id.parse::<i64>().ok();
            if let Some(numeric_id) = folder_id_num {
                log::info!("Querying for folder_id: {}", numeric_id);
                sqlx::query_as(r#"
                    SELECT id, uuid, clip_type,
                           CASE WHEN clip_type = 'image' THEN content ELSE '' END as content,
                           text_preview, content_hash,
                           folder_id, is_deleted, source_app, source_icon, metadata,
                           created_at, last_accessed, last_pasted_at, is_pinned,
                           subtype, note, paste_count
                    FROM clips WHERE folder_id = ?
                    ORDER BY is_pinned DESC, created_at DESC LIMIT ? OFFSET ?
                "#)
                .bind(numeric_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool).await.map_err(|e| e.to_string())?
            } else {
                log::info!("Unknown folder_id, returning empty");
                Vec::new()
            }
        }
        None => {
            log::info!("Querying for items, offset: {}, limit: {}", offset, limit);
            sqlx::query_as(r#"
                SELECT id, uuid, clip_type,
                       CASE WHEN clip_type = 'image' THEN content ELSE '' END as content,
                       text_preview, content_hash,
                       folder_id, is_deleted, source_app, source_icon, metadata,
                       created_at, last_accessed, last_pasted_at, is_pinned,
                       subtype, note, paste_count
                FROM clips
                ORDER BY created_at DESC LIMIT ? OFFSET ?
            "#)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool).await.map_err(|e| e.to_string())?
        }
    };

    log::info!("DB: Found {} clips", clips.len());

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
            })
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn paste_clip(id: String, app: AppHandle, window: tauri::WebviewWindow, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    let clip: Option<Clip> = sqlx::query_as(r#"SELECT * FROM clips WHERE uuid = ?"#)
        .bind(&id)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            // Synchronize clipboard access across the app
            let _guard = crate::clipboard::CLIPBOARD_SYNC.lock().await;

            let content_hash = clip.content_hash.clone();
            let uuid = clip.uuid.clone();

            // Stop monitor
            if let Err(e) = stop_listening().await {
                 log::error!("Failed to stop listener: {}", e);
            }

            let mut final_res = Ok(());

            if clip.clip_type == "image" {
                crate::clipboard::set_ignore_hash(content_hash.clone());
                crate::clipboard::set_last_stable_hash(content_hash.clone());

                log::debug!("Frontend handled image. Skipping backend write.");

            } else {
                let content_str = String::from_utf8_lossy(&clip.content).to_string();
                crate::clipboard::set_ignore_hash(content_hash.clone());
                crate::clipboard::set_last_stable_hash(content_hash.clone());

                let mut last_err = String::new();
                for i in 0..5 {
                    // write_text is public function
                    match write_text(content_str.clone()).await {
                        Ok(_) => { last_err.clear(); break; },
                        Err(e) => {
                            last_err = e.to_string();
                            log::warn!("ClipPaste clipboard write (text) attempt {} failed: {}. Retrying...", i+1, last_err);
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
                if !last_err.is_empty() {
                    final_res = Err(format!("Failed to set clipboard text: {}", last_err));
                }
            }

            // Track when this clip was last pasted + increment paste count
            let _ = sqlx::query(r#"UPDATE clips SET last_pasted_at = CURRENT_TIMESTAMP, paste_count = paste_count + 1 WHERE uuid = ?"#)
                .bind(&uuid)
                .execute(pool)
                .await;

            // Restart monitor
            let app_clone = app.clone();
            if let Err(e) = start_listening(app_clone).await {
                log::error!("Failed to restart listener: {}", e);
            }

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

    let clip: Option<Clip> = sqlx::query_as(r#"SELECT * FROM clips WHERE uuid = ?"#)
        .bind(&id)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            let _guard = crate::clipboard::CLIPBOARD_SYNC.lock().await;

            let content_hash = clip.content_hash.clone();

            if let Err(e) = stop_listening().await {
                log::error!("Failed to stop listener: {}", e);
            }

            // Only write text clips — images are handled by frontend navigator.clipboard
            if clip.clip_type != "image" {
                let content_str = String::from_utf8_lossy(&clip.content).to_string();
                crate::clipboard::set_ignore_hash(content_hash.clone());
                crate::clipboard::set_last_stable_hash(content_hash);

                let mut last_err = String::new();
                for i in 0..5 {
                    match write_text(content_str.clone()).await {
                        Ok(_) => { last_err.clear(); break; },
                        Err(e) => {
                            last_err = e.to_string();
                            log::warn!("copy_clip clipboard write attempt {} failed: {}. Retrying...", i+1, last_err);
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
                if !last_err.is_empty() {
                    if let Err(e) = start_listening(app.clone()).await {
                        log::error!("Failed to restart listener: {}", e);
                    }
                    return Err(format!("Failed to copy text: {}", last_err));
                }
            } else {
                crate::clipboard::set_ignore_hash(content_hash.clone());
                crate::clipboard::set_last_stable_hash(content_hash);
            }

            if let Err(e) = start_listening(app.clone()).await {
                log::error!("Failed to restart listener: {}", e);
            }

            // Does NOT hide window or simulate paste
            Ok(())
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn paste_text(content: String, app: AppHandle, window: tauri::WebviewWindow, _db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let _guard = crate::clipboard::CLIPBOARD_SYNC.lock().await;

    // Compute hash so the monitor ignores this self-write
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    crate::clipboard::set_ignore_hash(content_hash.clone());
    crate::clipboard::set_last_stable_hash(content_hash);

    if let Err(e) = stop_listening().await {
        log::error!("Failed to stop listener: {}", e);
    }

    let mut last_err = String::new();
    for i in 0..5 {
        match write_text(content.clone()).await {
            Ok(_) => { last_err.clear(); break; }
            Err(e) => {
                last_err = e.to_string();
                log::warn!("paste_text clipboard write attempt {} failed: {}. Retrying...", i + 1, last_err);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    if let Err(e) = start_listening(app.clone()).await {
        log::error!("Failed to restart listener: {}", e);
    }

    if !last_err.is_empty() {
        return Err(format!("Failed to set clipboard text: {}", last_err));
    }

    let _ = window.emit("clipboard-write", &content);

    check_auto_paste_and_hide(&window);

    Ok(())
}

#[tauri::command]
pub async fn delete_clip(id: String, hard_delete: bool, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    // If hard deleting an image clip, also remove the file from disk
    if hard_delete {
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

        sqlx::query(r#"DELETE FROM clips WHERE uuid = ?"#)
            .bind(&id)
            .execute(pool).await.map_err(|e| e.to_string())?;
    } else {
        sqlx::query(r#"DELETE FROM clips WHERE uuid = ?"#)
            .bind(&id)
            .execute(pool).await.map_err(|e| e.to_string())?;
    }

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

    // Search ALL clips (cross-folder), but collect folder_id for priority sorting
    let matched: Vec<(String, Option<i64>)> = {
        let cache = crate::clipboard::SEARCH_CACHE.read();
        cache.iter()
            .filter(|(_, preview, _)| {
                // All words must be present (AND logic) -- supports multi-word search
                query_words.iter().all(|word| preview.contains(word))
            })
            .map(|(uuid, _, fid)| (uuid.clone(), *fid))
            .collect()
    };

    // Sort: items in the selected folder first, then items in any folder, then unfiled
    let mut matched = matched;
    matched.sort_by_key(|(_, fid)| {
        if let Some(target_fid) = folder_filter {
            if *fid == Some(target_fid) { 0 } else if fid.is_some() { 1 } else { 2 }
        } else {
            // "All" view: prioritize clips in folders over unfiled
            if fid.is_some() { 0 } else { 1 }
        }
    });
    let matched: Vec<String> = matched.into_iter()
        .take(limit as usize)
        .map(|(uuid, _)| uuid)
        .collect();

    let mut clips: Vec<Clip> = if matched.is_empty() {
        Vec::new()
    } else {
        let placeholders: String = matched.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, uuid, clip_type, '' as content, text_preview, content_hash,
                    folder_id, is_deleted, source_app, source_icon, metadata,
                    created_at, last_accessed, last_pasted_at, is_pinned,
                    subtype, note, paste_count
             FROM clips WHERE uuid IN ({})",
            placeholders
        );
        let mut q = sqlx::query_as::<_, Clip>(&sql);
        for uuid in &matched {
            q = q.bind(uuid);
        }
        q.fetch_all(pool).await.map_err(|e| e.to_string())?
    };

    // Sort by relevance: same-folder > any folder > unfiled > starts_with > exact > created_at DESC
    clips.sort_by(|a, b| {
        // 1. Folder priority: selected folder > has folder > unfiled
        let folder_rank = |clip: &Clip| -> u8 {
            if let Some(target_fid) = folder_filter {
                if clip.folder_id == Some(target_fid) { 0 } else if clip.folder_id.is_some() { 1 } else { 2 }
            } else {
                if clip.folder_id.is_some() { 0 } else { 1 }
            }
        };
        let a_rank = folder_rank(a);
        let b_rank = folder_rank(b);
        // 2. Relevance ranking
        let a_starts = a.text_preview.to_lowercase().starts_with(&query_lower);
        let b_starts = b.text_preview.to_lowercase().starts_with(&query_lower);
        let a_exact = a.text_preview.to_lowercase().contains(&query_lower);
        let b_exact = b.text_preview.to_lowercase().contains(&query_lower);

        a_rank.cmp(&b_rank)
            .then(b_starts.cmp(&a_starts))
            .then(b_exact.cmp(&a_exact))
            .then(b.created_at.cmp(&a.created_at))
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
    Ok(())
}
