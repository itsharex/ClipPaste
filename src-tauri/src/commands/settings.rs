use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use std::str::FromStr;
use std::sync::Arc;
use dark_light::Mode;
use crate::database::Database;

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
        let max_items = max_items.clamp(10, 100_000);
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('max_items', ?)"#)
            .bind(max_items.to_string())
            .execute(pool).await.ok();
    }

    if let Some(days) = settings.get("auto_delete_days").and_then(|v| v.as_i64()) {
        let days = days.clamp(1, 3650);
        sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('auto_delete_days', ?)"#)
            .bind(days.to_string())
            .execute(pool).await.ok();
    }

    if let Some(theme) = settings.get("theme").and_then(|v| v.as_str()) {
        if matches!(theme, "light" | "dark" | "system") {
            sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('theme', ?)"#)
                .bind(theme)
                .execute(pool).await.ok();
        }
    }

    if let Some(mica_effect) = settings.get("mica_effect").and_then(|v| v.as_str()) {
        if matches!(mica_effect, "clear" | "mica" | "mica_alt") {
            sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('mica_effect', ?)"#)
                .bind(mica_effect)
                .execute(pool).await.ok();
        }
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
        // Validate hotkey format before saving
        if Shortcut::from_str(hotkey).is_ok() {
            sqlx::query(r#"INSERT OR REPLACE INTO settings (key, value) VALUES ('hotkey', ?)"#)
                .bind(hotkey)
                .execute(pool).await.ok();
        } else {
            log::warn!("SETTINGS: Invalid hotkey format rejected: {}", hotkey);
        }
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

    // Reload settings cache after changes
    crate::clipboard::load_settings_cache(&db.pool).await;

    Ok(())
}

#[tauri::command]
pub async fn get_clipboard_history_size(db: tauri::State<'_, Arc<Database>>) -> Result<i64, String> {
    let pool = &db.pool;

    let count: i64 = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM clips"#)
        .fetch_one(pool).await.map_err(|e| e.to_string())?;
    Ok(count)
}

#[tauri::command]
pub async fn clear_clipboard_history(db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    // Only delete soft-deleted clips that are NOT in any folder
    // No-op: soft delete no longer used, all deletes are hard deletes now
    // Kept for API compatibility
    sqlx::query(r#"SELECT 1"#)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn clear_all_clips(app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    use tauri::Emitter;
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
