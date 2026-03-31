use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_x::{write_text, stop_listening, start_listening};
use sha2::{Digest, Sha256};

use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use std::str::FromStr;
use crate::database::Database;
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::models::{Clip, Folder, ClipboardItem, FolderItem};
use dark_light::Mode;
use dirs;
use std::io::{Read as IoRead, Write as IoWrite};



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
                    SELECT * FROM clips WHERE is_deleted = 0 AND folder_id = ?
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
                SELECT * FROM clips WHERE is_deleted = 0
                ORDER BY created_at DESC LIMIT ? OFFSET ?
            "#)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool).await.map_err(|e| e.to_string())?
        }
    };

    log::info!("DB: Found {} clips", clips.len());

    let items: Vec<ClipboardItem> = clips.iter().enumerate().map(|(idx, clip)| {
        let content_str = if preview_only && clip.clip_type == "image" {
            String::new()
        } else if clip.clip_type == "image" {
            // Image content is now a filename — read from disk
            let filename = String::from_utf8_lossy(&clip.content).to_string();
            let image_path = db.images_dir.join(&filename);
            match std::fs::read(&image_path) {
                Ok(bytes) => BASE64.encode(&bytes),
                Err(_) => String::new(),
            }
        } else {
            String::from_utf8_lossy(&clip.content).to_string()
        };

        if idx < 10 {
            log::trace!("{} Clip {}: type='{}', content_len={}", idx, clip.uuid, clip.clip_type, content_str.len());
        }

        ClipboardItem {
            id: clip.uuid.clone(),
            clip_type: clip.clip_type.clone(),
            content: content_str,
            preview: clip.text_preview.clone(),
            folder_id: clip.folder_id.map(|id| id.to_string()),
            created_at: clip.created_at.to_rfc3339(),
            source_app: clip.source_app.clone(),
            source_icon: clip.source_icon.clone(),
            metadata: clip.metadata.clone(),
            is_pinned: clip.is_pinned,
            subtype: clip.subtype.clone(),
            note: clip.note.clone(),
            paste_count: clip.paste_count,
        }
    }).collect();

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

                // Check auto_paste setting
                let auto_paste = sqlx::query_scalar::<_, String>(r#"SELECT value FROM settings WHERE key = 'auto_paste'"#)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None)
                    .and_then(|v| v.parse::<bool>().ok())
                    .unwrap_or(true); // Default true

                if auto_paste {
                    // Auto-Paste Logic
                    // 1. Hide window immediately to trigger focus switch to previous app
                    crate::animate_window_hide(&window, Some(Box::new(move || {
                        // 2. Callback executed AFTER window is hidden
                        #[cfg(target_os = "windows")]
                        {
                            // Small buffer to ensure OS focus switch is complete
                            std::thread::sleep(std::time::Duration::from_millis(200));
                            crate::clipboard::send_paste_input();
                        }
                    })));
                } else {
                     // If auto_paste is disabled, we still hide the window (as requested by original "copy to text field" intent,
                     // but maybe user just wants to copy?)
                     // Actually, if auto_paste is OFF, standard behavior for "Enter/Double Click" in clipboard managers is usually "Copy & Close".
                     crate::animate_window_hide(&window, None);
                }
            }
            final_res
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn paste_text(content: String, app: AppHandle, window: tauri::WebviewWindow, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;
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

    let auto_paste = sqlx::query_scalar::<_, String>(r#"SELECT value FROM settings WHERE key = 'auto_paste'"#)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    if auto_paste {
        crate::animate_window_hide(&window, Some(Box::new(move || {
            #[cfg(target_os = "windows")]
            {
                std::thread::sleep(std::time::Duration::from_millis(200));
                crate::clipboard::send_paste_input();
            }
        })));
    } else {
        crate::animate_window_hide(&window, None);
    }

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
        // Clean up FTS5 index
        let _ = sqlx::query("DELETE FROM clips_fts WHERE uuid = ?")
            .bind(&id).execute(pool).await;
    } else {
        sqlx::query(r#"UPDATE clips SET is_deleted = 1 WHERE uuid = ?"#)
            .bind(&id)
            .execute(pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}



#[tauri::command]
pub async fn move_to_folder(clip_id: String, folder_id: Option<String>, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    let folder_id = match folder_id {
        Some(id) => Some(id.parse::<i64>().map_err(|_| "Invalid folder ID")?),
        None => None,
    };

    sqlx::query(r#"UPDATE clips SET folder_id = ? WHERE uuid = ?"#)
        .bind(folder_id)
        .bind(&clip_id)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn create_folder(name: String, icon: Option<String>, color: Option<String>, db: tauri::State<'_, Arc<Database>>, window: tauri::WebviewWindow) -> Result<FolderItem, String> {
    let pool = &db.pool;

    // Check if folder with same name exists (excluding system folders if we wanted, but name uniqueness is good generally)
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM folders WHERE name = ?")
        .bind(&name)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    if exists.is_some() {
        return Err("A folder with this name already exists".to_string());
    }

    let id = sqlx::query(r#"INSERT INTO folders (name, icon, color) VALUES (?, ?, ?)"#)
        .bind(&name)
        .bind(icon.as_ref())
        .bind(color.as_ref())
        .execute(pool).await.map_err(|e| e.to_string())?
        .last_insert_rowid();

    let _ = window.emit("clipboard-change", ());

    Ok(FolderItem {
        id: id.to_string(),
        name,
        icon,
        color,
        is_system: false,
        item_count: 0,
    })
}

#[tauri::command]
pub async fn delete_folder(id: String, db: tauri::State<'_, Arc<Database>>, window: tauri::WebviewWindow) -> Result<(), String> {
    let pool = &db.pool;

    let folder_id: i64 = id.parse().map_err(|_| "Invalid folder ID")?;

    // Clean up image files for clips in this folder before deleting
    let image_clips: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT content FROM clips WHERE folder_id = ? AND clip_type = 'image'"
    ).bind(folder_id).fetch_all(pool).await.map_err(|e| e.to_string())?;
    for (content,) in &image_clips {
        let filename = String::from_utf8_lossy(content).to_string();
        let image_path = db.images_dir.join(&filename);
        if image_path.exists() { let _ = std::fs::remove_file(&image_path); }
    }

    // Hard-delete all clips in this folder first (user explicitly chose to delete the folder)
    sqlx::query(r#"DELETE FROM clips WHERE folder_id = ?"#)
        .bind(folder_id)
        .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(r#"DELETE FROM folders WHERE id = ?"#)
        .bind(folder_id)
        .execute(pool).await.map_err(|e| e.to_string())?;

    let _ = window.emit("clipboard-change", ());
    Ok(())
}

#[tauri::command]
pub async fn rename_folder(id: String, name: String, color: Option<String>, icon: Option<String>, db: tauri::State<'_, Arc<Database>>, window: tauri::WebviewWindow) -> Result<(), String> {
    let pool = &db.pool;

    let folder_id: i64 = id.parse().map_err(|_| "Invalid folder ID")?;

    // Check availability
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM folders WHERE name = ? AND id != ?")
        .bind(&name)
        .bind(folder_id)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    if exists.is_some() {
        return Err("A folder with this name already exists".to_string());
    }

    sqlx::query(r#"UPDATE folders SET name = ?, color = ?, icon = ? WHERE id = ?"#)
        .bind(name)
        .bind(color)
        .bind(icon)
        .bind(folder_id)
        .execute(pool).await.map_err(|e| e.to_string())?;

    // Emit event so main window knows to refresh
    let _ = window.emit("clipboard-change", ());
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
pub async fn reorder_folders(folder_ids: Vec<String>, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;
    for (idx, id) in folder_ids.iter().enumerate() {
        let folder_id: i64 = id.parse().map_err(|_| "Invalid folder ID")?;
        sqlx::query("UPDATE folders SET position = ? WHERE id = ?")
            .bind(idx as i64)
            .bind(folder_id)
            .execute(pool).await.map_err(|e| e.to_string())?;
    }
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

    let matched: Vec<String> = {
        let cache = crate::clipboard::SEARCH_CACHE.lock();
        cache.iter()
            .filter(|(_, preview, fid)| {
                if let Some(target_fid) = folder_filter {
                    if *fid != Some(target_fid) { return false; }
                }
                // All words must be present (AND logic) — supports multi-word search
                query_words.iter().all(|word| preview.contains(word))
            })
            .take(limit as usize)
            .map(|(uuid, _, _)| uuid.clone())
            .collect()
    };

    let clips: Vec<Clip> = if matched.is_empty() {
        Vec::new()
    } else {
        let placeholders: String = matched.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, uuid, clip_type, '' as content, text_preview, content_hash,
                    folder_id, is_deleted, source_app, source_icon, metadata,
                    created_at, last_accessed, last_pasted_at, is_pinned,
                    subtype, note, paste_count
             FROM clips WHERE uuid IN ({})
             ORDER BY created_at DESC",
            placeholders
        );
        let mut q = sqlx::query_as::<_, Clip>(&sql);
        for uuid in &matched {
            q = q.bind(uuid);
        }
        q.fetch_all(pool).await.map_err(|e| e.to_string())?
    };

    // Search results use text_preview instead of full content for speed.
    // Cards only display ~300 chars anyway. Full content loaded on paste.
    let items: Vec<ClipboardItem> = clips.iter().map(|clip| {
        let content_str = if clip.clip_type == "image" {
            // Thumbnail or empty — images matched by text_preview only
            String::new()
        } else {
            clip.text_preview.clone()
        };

        ClipboardItem {
            id: clip.uuid.clone(),
            clip_type: clip.clip_type.clone(),
            content: content_str,
            preview: clip.text_preview.clone(),
            folder_id: clip.folder_id.map(|id| id.to_string()),
            created_at: clip.created_at.to_rfc3339(),
            source_app: clip.source_app.clone(),
            source_icon: clip.source_icon.clone(),
            metadata: clip.metadata.clone(),
            is_pinned: clip.is_pinned,
            subtype: clip.subtype.clone(),
            note: clip.note.clone(),
            paste_count: clip.paste_count,
        }
    }).collect();

    Ok(items)
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

#[tauri::command]
pub async fn get_folders(db: tauri::State<'_, Arc<Database>>) -> Result<Vec<FolderItem>, String> {
    let pool = &db.pool;

    let folders: Vec<Folder> = sqlx::query_as(r#"SELECT * FROM folders ORDER BY position, id"#)
        .fetch_all(pool).await.map_err(|e| e.to_string())?;

    // Get counts for all folders in one query
    let counts: Vec<(i64, i64)> = sqlx::query_as(r#"
        SELECT folder_id, COUNT(*) as count
        FROM clips
        WHERE is_deleted = 0 AND folder_id IS NOT NULL
        GROUP BY folder_id
    "#)
    .fetch_all(pool).await.map_err(|e| e.to_string())?;

    // Create a map for easier lookup
    use std::collections::HashMap;
    let count_map: HashMap<i64, i64> = counts.into_iter().collect();

    let items: Vec<FolderItem> = folders.iter().map(|folder| {
        FolderItem {
            id: folder.id.to_string(),
            name: folder.name.clone(),
            icon: folder.icon.clone(),
            color: folder.color.clone(),
            is_system: folder.is_system,
            item_count: *count_map.get(&folder.id).unwrap_or(&0),
        }
    }).collect();


    Ok(items)
}

#[tauri::command]
pub async fn get_settings(app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<serde_json::Value, String> {
    use tauri_plugin_autostart::ManagerExt;
    let pool = &db.pool;

    let mut settings = serde_json::json!({
        "max_items": 1000,
        "auto_delete_days": 30,
        "startup_with_windows": false, // Default, will override below
        "show_in_taskbar": false,
        "hotkey": "Ctrl+Shift+V",
        "theme": "dark",
        "mica_effect": "clear",
        "auto_paste": true,
        "ignore_ghost_clips": false
    });

    if let Ok(rows) = sqlx::query_as::<_, (String, String)>(r#"SELECT key, value FROM settings"#)
        .fetch_all(pool).await
    {
        for (key, value) in rows {
            match key.as_str() {
                "mica_effect" | "theme" | "hotkey" => {
                    settings[&key] = serde_json::json!(value);
                }
                "ignore_ghost_clips" | "auto_paste" => {
                    if let Ok(b) = value.parse::<bool>() {
                        settings[&key] = serde_json::json!(b);
                    }
                }
                "max_items" | "auto_delete_days" => {
                    if let Ok(num) = value.parse::<i64>() {
                        settings[&key] = serde_json::json!(num);
                    }
                }
                _ => {}
            }
        }
    }



    // Check actual autostart status
    if let Ok(is_enabled) = app.autolaunch().is_enabled() {
        settings["startup_with_windows"] = serde_json::json!(is_enabled);
        log::info!("autostart enabled: {}", is_enabled);
    } else {
        log::info!("autostart not enabled");
    }

    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: serde_json::Value, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let pool = &db.pool;

    if let Some(max_items) = settings.get("max_items").and_then(|v| v.as_i64()) {
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('max_items', ?)"#)
            .bind(max_items.to_string())
            .execute(pool).await.ok();
    }

    if let Some(days) = settings.get("auto_delete_days").and_then(|v| v.as_i64()) {
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('auto_delete_days', ?)"#)
            .bind(days.to_string())
            .execute(pool).await.ok();
    }

    if let Some(theme) = settings.get("theme").and_then(|v| v.as_str()) {
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('theme', ?)"#)
            .bind(theme)
            .execute(pool).await.ok();
    }

    if let Some(mica_effect) = settings.get("mica_effect").and_then(|v| v.as_str()) {
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('mica_effect', ?)"#)
            .bind(mica_effect)
            .execute(pool).await.ok();
    }

    // Always re-apply window effect when theme or mica_effect might have changed
    let theme_str = settings.get("theme").and_then(|v| v.as_str()).unwrap_or("system");
    let mica_effect = settings.get("mica_effect").and_then(|v| v.as_str()).unwrap_or("clear");
    if let Some(win) = app.get_webview_window("main") {
        // get current system theme
        let current_theme = if theme_str == "light" {
            tauri::Theme::Light
        } else if theme_str == "dark" {
            tauri::Theme::Dark
        } else {
            let mode = dark_light::detect().map_err(|e| {
                log::error!("THEME: Failed to detect system theme: {:?} via dark_light::detect()", e);
                e.to_string()
            })?;

            let theme2 = match mode {
                Mode::Dark => tauri::Theme::Dark,
                Mode::Light => tauri::Theme::Light,
                _ => tauri::Theme::Light,
            };

            log::info!("THEME: win.theme(): {:?}, dark_light::detectd(): {:?}", win.theme(), theme2);

            // sometimes win.theme() is not right. don't why for now..
            // win.theme().unwrap_or_else(|err| {
            //     log::error!("THEME: Failed to get system theme: {:?}, defaulting to Light", err);
            //     tauri::Theme::Light
            // })
            theme2
        };
        log::info!("THEME:Applying window effect: {} with theme: {:?} (setting:{:?}", mica_effect, current_theme, theme_str);
        crate::apply_window_effect(&win, mica_effect, &current_theme);
    }


    if let Some(hotkey) = settings.get("hotkey").and_then(|v| v.as_str()) {
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('hotkey', ?)"#)
            .bind(hotkey)
            .execute(pool).await.ok();
    }

    if let Some(auto_paste) = settings.get("auto_paste").and_then(|v| v.as_bool()) {
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('auto_paste', ?)"#)
            .bind(auto_paste.to_string())
            .execute(pool).await.ok();
    }

    if let Some(ignore_ghost) = settings.get("ignore_ghost_clips").and_then(|v| v.as_bool()) {
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('ignore_ghost_clips', ?)"#)
            .bind(ignore_ghost.to_string())
            .execute(pool).await.ok();
    }

    if let Some(startup) = settings.get("startup_with_windows").and_then(|v| v.as_bool()) {
        let current_state = app.autolaunch().is_enabled().unwrap_or(false);
        if startup != current_state {
             if startup {
                 if let Err(e) = app.autolaunch().enable() {
                     log::warn!("Failed to enable autostart: {}", e);
                 }
             } else {
                 if let Err(e) = app.autolaunch().disable() {
                     log::warn!("Failed to disable autostart: {}", e);
                 }
             }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn hide_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ping() -> Result<String, String> {
    Ok("pong".to_string())
}

#[tauri::command]
pub fn test_log() -> Result<String, String> {
    log::trace!("[TEST] Trace level log");
    log::debug!("[TEST] Debug level log");
    log::info!("[TEST] Info level log");
    log::warn!("[TEST] Warn level log");
    log::error!("[TEST] Error level log");
    Ok("Logs emitted - check console".to_string())
}

#[tauri::command]
pub async fn get_clipboard_history_size(db: tauri::State<'_, Arc<Database>>) -> Result<i64, String> {
    let pool = &db.pool;

    let count: i64 = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM clips WHERE is_deleted = 0"#)
        .fetch_one(pool).await.map_err(|e| e.to_string())?;
    Ok(count)
}

#[tauri::command]
pub async fn clear_clipboard_history(db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    // Only delete soft-deleted clips that are NOT in any folder
    sqlx::query(r#"DELETE FROM clips WHERE is_deleted = 1 AND folder_id IS NULL"#)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn clear_all_clips(app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    // Clean up image files before deleting
    let image_clips: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT content FROM clips WHERE folder_id IS NULL AND clip_type = 'image'"
    ).fetch_all(pool).await.map_err(|e| e.to_string())?;
    for (content,) in &image_clips {
        let filename = String::from_utf8_lossy(content).to_string();
        let image_path = db.images_dir.join(&filename);
        if image_path.exists() { let _ = std::fs::remove_file(&image_path); }
    }

    sqlx::query(r#"DELETE FROM clips WHERE folder_id IS NULL"#)
        .execute(pool).await.map_err(|e| e.to_string())?;

    // Notify main window to refresh
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("clipboard-change", ());
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_duplicate_clips(db: tauri::State<'_, Arc<Database>>) -> Result<i64, String> {
    let pool = &db.pool;

    // Only remove duplicates from clips that are NOT in any folder
    // Folder items are protected and can only be deleted manually
    let result = sqlx::query(r#"
        DELETE FROM clips
        WHERE folder_id IS NULL
        AND id NOT IN (
            SELECT MIN(id)
            FROM clips
            WHERE folder_id IS NULL
            GROUP BY content_hash
        )
    "#)
    .execute(pool).await.map_err(|e| e.to_string())?;

    Ok(result.rows_affected() as i64)
}

#[tauri::command]
pub async fn register_global_shortcut(hotkey: String, window: tauri::WebviewWindow) -> Result<(), String> {
    use tauri_plugin_global_shortcut::ShortcutState;

    let app = window.app_handle();
    let shortcut = Shortcut::from_str(&hotkey).map_err(|e| format!("Invalid hotkey: {:?}", e))?;

    // Unregister all existing shortcuts first
    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!("Failed to unregister existing shortcuts: {:?}", e);
    }

    // Get the main window for the handler
    let main_window = app.get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    // Register the new shortcut with the window show handler
    let win_clone = main_window.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            crate::position_window_at_bottom(&win_clone);
            let _ = win_clone.show();
            let _ = win_clone.set_focus();
        }
    }) {
        return Err(format!("Failed to register hotkey: {:?}", e));
    }

    log::info!("Registered global shortcut: {}", hotkey);
    Ok(())
}

#[tauri::command]
pub fn set_dragging(dragging: bool) {
    use std::sync::atomic::Ordering;
    crate::IS_DRAGGING.store(dragging, Ordering::SeqCst);
    log::debug!("Dragging state set to: {}", dragging);
}

#[tauri::command]
pub async fn focus_window(app: AppHandle, label: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&label) {
        if let Err(e) = window.unminimize() {
            log::warn!("Failed to unminimize window {}: {:?}", label, e);
        }
        if let Err(e) = window.show() {
            log::warn!("Failed to show window {}: {:?}", label, e);
        }
        if let Err(e) = window.set_focus() {
            log::warn!("Failed to focus window {}: {:?}", label, e);
        }

        Ok(())
    } else {
        Err(format!("Window {} not found", label))
    }
}

#[tauri::command]
pub fn show_window(window: tauri::WebviewWindow) -> Result<(), String> {
    crate::position_window_at_bottom(&window);
    if let Err(e) = window.show() {
        return Err(format!("Failed to show window: {:?}", e));
    }
    if let Err(e) = window.set_focus() {
        return Err(format!("Failed to focus window: {:?}", e));
    }
    Ok(())
}

#[tauri::command]
pub async fn add_ignored_app(app_name: String, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    db.add_ignored_app(&app_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_ignored_app(app_name: String, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    db.remove_ignored_app(&app_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ignored_apps(db: tauri::State<'_, Arc<Database>>) -> Result<Vec<String>, String> {
    db.get_ignored_apps().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_file() -> Result<String, String> {
    use std::process::Command;
    #[cfg(target_os = "windows")]
    {
        let ps_script = "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Filter = 'Executables (*.exe)|*.exe|All files (*.*)|*.*'; $null = $d.ShowDialog(); $d.FileName";
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", ps_script])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                return Err("No file selected".to_string());
            }
            Ok(path)
        } else {
            Err("Failed to open file picker".to_string())
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("osascript")
            .args(["-e", "POSIX path of (choose file)"])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() { return Err("No file selected".to_string()); }
            Ok(path)
        } else {
            Err("No file selected".to_string())
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let output = Command::new("zenity")
            .args(["--file-selection"])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() { return Err("No file selected".to_string()); }
            Ok(path)
        } else {
            Err("No file selected".to_string())
        }
    }
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{
            FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH, FOS_PICKFOLDERS,
        };

        let hwnd = app
            .get_webview_window("main")
            .and_then(|w| w.hwnd().ok())
            .map(|h| HWND(h.0 as _))
            .unwrap_or(HWND(std::ptr::null_mut()));

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            // Use a closure so CoUninitialize is always called regardless of early returns
            let result = (|| -> Result<String, String> {
                let dialog: IFileOpenDialog =
                    CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| format!("Failed to create dialog: {}", e))?;

                let options = dialog.GetOptions().map_err(|e| e.to_string())?;
                dialog
                    .SetOptions(options | FOS_PICKFOLDERS)
                    .map_err(|e| e.to_string())?;

                dialog.Show(Some(hwnd)).map_err(|_| "No folder selected".to_string())?;

                let item = dialog.GetResult().map_err(|e| e.to_string())?;
                let pwstr = item
                    .GetDisplayName(SIGDN_FILESYSPATH)
                    .map_err(|e| e.to_string())?;
                let path = pwstr.to_string().map_err(|e| e.to_string())?;
                CoTaskMemFree(Some(pwstr.0 as _));
                Ok(path)
            })();

            CoUninitialize();
            result
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        use std::process::Command;
        let output = Command::new("osascript")
            .args(["-e", "POSIX path of (choose folder)"])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() { return Err("No folder selected".to_string()); }
            Ok(path)
        } else {
            Err("No folder selected".to_string())
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        use std::process::Command;
        let output = Command::new("zenity")
            .args(["--file-selection", "--directory"])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() { return Err("No folder selected".to_string()); }
            Ok(path)
        } else {
            Err("No folder selected".to_string())
        }
    }
}

#[tauri::command]
pub fn get_layout_config() -> serde_json::Value {
    serde_json::json!({
        "window_height": crate::constants::WINDOW_HEIGHT,
    })
}

fn get_config_path() -> std::path::PathBuf {
    let default_config_dir = match dirs::config_dir() {
        Some(path) => path.join("ClipPaste"),
        None => std::env::current_dir().unwrap_or(std::path::PathBuf::from(".")).join("ClipPaste"),
    };
    default_config_dir.join("config.json")
}

fn get_default_data_dir() -> std::path::PathBuf {
    let current_dir = std::env::current_dir().unwrap_or(std::path::PathBuf::from("."));
    match dirs::data_dir() {
        Some(path) => path.join("ClipPaste"),
        None => current_dir.join("ClipPaste"),
    }
}

#[tauri::command]
pub fn get_data_directory() -> Result<String, String> {
    let config_path = get_config_path();
    if let Ok(config_content) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&config_content) {
            if let Some(custom_path) = config.get("data_directory").and_then(|v| v.as_str()) {
                return Ok(custom_path.to_string());
            }
        }
    }
    
    // Return default location
    let default_dir = get_default_data_dir();
    Ok(default_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn set_data_directory(
    new_path: String,
    _db: tauri::State<'_, Arc<Database>>,
    app: AppHandle,
) -> Result<(), String> {
    use std::path::PathBuf;

    let new_path_buf = PathBuf::from(&new_path);

    // Security: reject relative paths
    if !new_path_buf.is_absolute() {
        return Err("Path must be absolute".to_string());
    }

    // Security: reject UNC/network paths
    if new_path.starts_with("\\\\") || new_path.starts_with("//") {
        return Err("Network paths are not supported".to_string());
    }

    // Security: reject path traversal
    let path_str = new_path_buf.to_string_lossy();
    if path_str.contains("..") {
        return Err("Path traversal is not allowed".to_string());
    }

    // Validate path exists or can be created
    if !new_path_buf.exists() {
        if let Some(parent) = new_path_buf.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create directory: {}", e))?;
        } else {
            return Err("Invalid path".to_string());
        }
    }
    
    // Get current DB path (read from config or use default)
    let config_path = get_config_path();
    let current_data_dir = if let Ok(config_content) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&config_content) {
            if let Some(custom_path) = config.get("data_directory").and_then(|v| v.as_str()) {
                PathBuf::from(custom_path)
            } else {
                get_default_data_dir()
            }
        } else {
            get_default_data_dir()
        }
    } else {
        get_default_data_dir()
    };
    
    let current_db_path = current_data_dir.join("clipboard.db");
    let new_db_path = new_path_buf.join("clipboard.db");
    
    // If DB exists in current location and new location is different, migrate it
    if current_db_path.exists() && current_db_path != new_db_path {
        log::info!("Migrating DB from {:?} to {:?}", current_db_path, new_db_path);
        
        // Ensure new directory exists
        std::fs::create_dir_all(&new_path_buf).map_err(|e| format!("Cannot create directory: {}", e))?;
        
        // Copy DB file
        std::fs::copy(&current_db_path, &new_db_path)
            .map_err(|e| format!("Failed to copy database: {}", e))?;

        // Also migrate images directory
        let current_images_dir = current_data_dir.join("images");
        let new_images_dir = new_path_buf.join("images");
        if current_images_dir.exists() {
            std::fs::create_dir_all(&new_images_dir).ok();
            if let Ok(entries) = std::fs::read_dir(&current_images_dir) {
                for entry in entries.flatten() {
                    let dest = new_images_dir.join(entry.file_name());
                    let _ = std::fs::copy(entry.path(), dest);
                }
            }
            log::info!("Images directory migrated successfully");
        }

        log::info!("Database migrated successfully");
    }
    
    // Save config
    let config_path = get_config_path();
    if let Some(config_dir) = config_path.parent() {
        std::fs::create_dir_all(config_dir).ok();
    }
    
    let config = serde_json::json!({
        "data_directory": new_path
    });
    
    let config_json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&config_path, config_json)
        .map_err(|e| format!("Failed to save config: {}", e))?;
    
    log::info!("Data directory set to: {}", new_path);
    
    // Notify frontend that restart is needed
    let _ = app.emit("data-directory-changed", &serde_json::json!({
        "message": "Data directory changed. Please restart the application.",
        "new_path": new_path
    }));

    Ok(())
}

#[tauri::command]
pub async fn export_data(app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<String, String> {
    // Let user pick save location
    #[cfg(target_os = "windows")]
    let save_path = {
        let ps_script = r#"Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.SaveFileDialog; $d.Filter = 'Zip Archive (*.zip)|*.zip'; $d.FileName = 'ClipPaste-backup.zip'; $null = $d.ShowDialog(); $d.FileName"#;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", ps_script])
            .output()
            .map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Export cancelled".to_string()); }
        path
    };
    #[cfg(target_os = "macos")]
    let save_path = {
        let output = std::process::Command::new("osascript")
            .args(["-e", r#"POSIX path of (choose file name with prompt "Export ClipPaste backup" default name "ClipPaste-backup.zip")"#])
            .output()
            .map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Export cancelled".to_string()); }
        path
    };
    #[cfg(target_os = "linux")]
    let save_path = {
        let output = std::process::Command::new("zenity")
            .args(["--file-selection", "--save", "--filename=ClipPaste-backup.zip"])
            .output()
            .map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Export cancelled".to_string()); }
        path
    };

    // Checkpoint WAL to ensure all data is in the main DB file
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&db.pool).await;

    let data_dir = db.images_dir.parent().unwrap();
    let db_path = data_dir.join("clipboard.db");
    let images_dir = &db.images_dir;

    let file = std::fs::File::create(&save_path)
        .map_err(|e| format!("Failed to create zip: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Add DB file
    if db_path.exists() {
        let mut db_file = std::fs::File::open(&db_path)
            .map_err(|e| format!("Failed to read DB: {}", e))?;
        let mut buf = Vec::new();
        db_file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        zip.start_file("clipboard.db", options).map_err(|e| e.to_string())?;
        zip.write_all(&buf).map_err(|e| e.to_string())?;
    }

    // Add images
    if images_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(images_dir) {
            for entry in entries.flatten() {
                if let Ok(mut f) = std::fs::File::open(entry.path()) {
                    let name = format!("images/{}", entry.file_name().to_string_lossy());
                    let mut buf = Vec::new();
                    if f.read_to_end(&mut buf).is_ok() {
                        let _ = zip.start_file(&name, options);
                        let _ = zip.write_all(&buf);
                    }
                }
            }
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    log::info!("Exported backup to: {}", save_path);
    Ok(save_path)
}

#[tauri::command]
pub async fn import_data(app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    // Let user pick zip file
    #[cfg(target_os = "windows")]
    let zip_path = {
        let ps_script = r#"Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Filter = 'Zip Archive (*.zip)|*.zip'; $null = $d.ShowDialog(); $d.FileName"#;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", ps_script])
            .output()
            .map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Import cancelled".to_string()); }
        path
    };
    #[cfg(target_os = "macos")]
    let zip_path = {
        let output = std::process::Command::new("osascript")
            .args(["-e", r#"POSIX path of (choose file of type {"zip"} with prompt "Import ClipPaste backup")"#])
            .output()
            .map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Import cancelled".to_string()); }
        path
    };
    #[cfg(target_os = "linux")]
    let zip_path = {
        let output = std::process::Command::new("zenity")
            .args(["--file-selection", "--file-filter=*.zip"])
            .output()
            .map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Import cancelled".to_string()); }
        path
    };

    let data_dir = db.images_dir.parent().unwrap().to_path_buf();

    let file = std::fs::File::open(&zip_path)
        .map_err(|e| format!("Failed to open zip: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid zip: {}", e))?;

    // Validate: must contain clipboard.db
    let has_db = (0..archive.len()).any(|i| {
        archive.by_index(i).map(|f| f.name() == "clipboard.db").unwrap_or(false)
    });
    if !has_db {
        return Err("Invalid backup: clipboard.db not found in zip".to_string());
    }

    // Extract all files
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();

        // Security: reject path traversal
        if name.contains("..") { continue; }

        let out_path = data_dir.join(&name);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&out_path, &buf)
            .map_err(|e| format!("Failed to write {}: {}", name, e))?;
    }

    log::info!("Imported backup from: {}", zip_path);

    // Notify frontend to restart
    let _ = app.emit("data-directory-changed", &serde_json::json!({
        "message": "Backup imported. Please restart the application.",
        "new_path": data_dir.to_string_lossy()
    }));

    Ok(())
}
