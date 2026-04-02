use tauri::{AppHandle, Emitter, Manager};
use std::sync::Arc;
use std::io::{Read as IoRead, Write as IoWrite};
use crate::database::Database;
use crate::models::{Clip, ClipboardItem};
use crate::utils;
use super::helpers::clip_to_item_async;

#[tauri::command]
pub fn get_data_directory() -> Result<String, String> {
    let config_path = utils::get_config_path();
    if let Ok(config_content) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&config_content) {
            if let Some(custom_path) = config.get("data_directory").and_then(|v| v.as_str()) {
                return Ok(custom_path.to_string());
            }
        }
    }

    // Return default location
    let default_dir = utils::get_default_data_dir();
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
    let config_path = utils::get_config_path();
    let current_data_dir = if let Ok(config_content) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&config_content) {
            if let Some(custom_path) = config.get("data_directory").and_then(|v| v.as_str()) {
                PathBuf::from(custom_path)
            } else {
                utils::get_default_data_dir()
            }
        } else {
            utils::get_default_data_dir()
        }
    } else {
        utils::get_default_data_dir()
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
    let config_path = utils::get_config_path();
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
pub async fn pick_file() -> Result<String, String> {
    use std::process::Command;
    #[cfg(target_os = "windows")]
    let path = {
        let ps_script = "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Filter = 'Executables (*.exe)|*.exe|All files (*.*)|*.*'; $null = $d.ShowDialog(); $d.FileName";
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", ps_script])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if p.is_empty() {
                return Err("No file selected".to_string());
            }
            p
        } else {
            return Err("Failed to open file picker".to_string());
        }
    };
    #[cfg(target_os = "macos")]
    let path = {
        let output = Command::new("osascript")
            .args(["-e", "POSIX path of (choose file)"])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if p.is_empty() { return Err("No file selected".to_string()); }
            p
        } else {
            return Err("No file selected".to_string());
        }
    };
    #[cfg(target_os = "linux")]
    let path = {
        let output = Command::new("zenity")
            .args(["--file-selection"])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if p.is_empty() { return Err("No file selected".to_string()); }
            p
        } else {
            return Err("No file selected".to_string());
        }
    };

    // Sanitize: reject path traversal and control characters
    if path.contains("..") || path.chars().any(|c| c.is_control()) {
        return Err("Invalid file path".to_string());
    }

    Ok(path)
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

#[tauri::command]
pub async fn export_data(_app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<String, String> {
    // Let user pick save location (spawn blocking to avoid Tokio stall)
    #[cfg(target_os = "windows")]
    let save_path = {
        let ps_script = r#"Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.SaveFileDialog; $f.Filter = 'Zip Archive (*.zip)|*.zip'; $f.FileName = 'ClipPaste-backup.zip'; if ($f.ShowDialog() -eq 'OK') { $f.FileName } else { '' }"#;
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-STA", "-Command", ps_script])
                .output()
        }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Export cancelled".to_string()); }
        path
    };
    #[cfg(target_os = "macos")]
    let save_path = {
        let output = tokio::task::spawn_blocking(|| {
            std::process::Command::new("osascript")
                .args(["-e", r#"POSIX path of (choose file name with prompt "Export ClipPaste backup" default name "ClipPaste-backup.zip")"#])
                .output()
        }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Export cancelled".to_string()); }
        path
    };
    #[cfg(target_os = "linux")]
    let save_path = {
        let output = tokio::task::spawn_blocking(|| {
            std::process::Command::new("zenity")
                .args(["--file-selection", "--save", "--filename=ClipPaste-backup.zip"])
                .output()
        }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Export cancelled".to_string()); }
        path
    };

    // Checkpoint WAL to ensure all data is in the main DB file
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&db.pool).await;

    let data_dir = db.images_dir.parent()
        .ok_or_else(|| "Cannot determine data directory".to_string())?
        .to_path_buf();
    let db_path = data_dir.join("clipboard.db");
    let images_dir = db.images_dir.clone();
    let save_path_clone = save_path.clone();

    // Copy DB to temp file first to avoid SQLite lock conflicts
    let temp_db = std::env::temp_dir().join("clippaste-export-temp.db");
    std::fs::copy(&db_path, &temp_db)
        .map_err(|e| format!("Failed to copy DB for export: {}", e))?;

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let file = std::fs::File::create(&save_path_clone)
            .map_err(|e| format!("Failed to create zip: {}", e))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Add DB file from temp copy
        if temp_db.exists() {
            let mut db_file = std::fs::File::open(&temp_db)
                .map_err(|e| format!("Failed to read DB: {}", e))?;
            let mut buf = Vec::new();
            db_file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            zip.start_file("clipboard.db", options).map_err(|e| e.to_string())?;
            zip.write_all(&buf).map_err(|e| e.to_string())?;
            drop(db_file);
            let _ = std::fs::remove_file(&temp_db);
        }

        // Add images
        if images_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&images_dir) {
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
        Ok(())
    }).await.map_err(|e| e.to_string())??;

    // Verify zip integrity by attempting to open and read the archive
    let verify_path = save_path.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let file = std::fs::File::open(&verify_path)
            .map_err(|e| format!("Export verification failed: cannot open zip: {}", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Export verification failed: invalid zip: {}", e))?;
        let has_db = (0..archive.len()).any(|i| {
            archive.by_index_raw(i).map(|f| f.name() == "clipboard.db").unwrap_or(false)
        });
        if !has_db {
            return Err("Export verification failed: clipboard.db not found in archive".to_string());
        }
        Ok(())
    }).await.map_err(|e| e.to_string())??;

    log::info!("Exported backup to: {}", save_path);
    Ok(save_path)
}

#[tauri::command]
pub async fn import_data(app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    // Let user pick zip file (spawn blocking to avoid Tokio stall)
    #[cfg(target_os = "windows")]
    let zip_path = {
        let ps_script = r#"Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Filter = 'Zip Archive (*.zip)|*.zip'; if ($f.ShowDialog() -eq 'OK') { $f.FileName } else { '' }"#;
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-STA", "-Command", ps_script])
                .output()
        }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Import cancelled".to_string()); }
        path
    };
    #[cfg(target_os = "macos")]
    let zip_path = {
        let output = tokio::task::spawn_blocking(|| {
            std::process::Command::new("osascript")
                .args(["-e", r#"POSIX path of (choose file of type {"zip"} with prompt "Import ClipPaste backup")"#])
                .output()
        }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Import cancelled".to_string()); }
        path
    };
    #[cfg(target_os = "linux")]
    let zip_path = {
        let output = tokio::task::spawn_blocking(|| {
            std::process::Command::new("zenity")
                .args(["--file-selection", "--file-filter=*.zip"])
                .output()
        }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { return Err("Import cancelled".to_string()); }
        path
    };

    let data_dir = db.images_dir.parent().unwrap().to_path_buf();
    let data_dir_clone = data_dir.clone();

    tokio::task::spawn_blocking(move || -> Result<(), String> {
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

        // Extract all files with strict path validation
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().to_string();

            // Security: reject path traversal and suspicious names
            if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
                log::warn!("Import: skipping suspicious entry: {}", name);
                continue;
            }

            // Only allow known safe paths: clipboard.db and images/*
            let is_safe = name == "clipboard.db"
                || (name.starts_with("images/") && !name.contains(".."));
            if !is_safe {
                log::warn!("Import: skipping unexpected entry: {}", name);
                continue;
            }

            let out_path = data_dir_clone.join(&name);

            // Canonicalize check: resolved path must be inside data_dir
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let canonical = out_path.canonicalize().unwrap_or_else(|_| out_path.clone());
            let canonical_base = data_dir_clone.canonicalize().unwrap_or_else(|_| data_dir_clone.clone());
            if !canonical.starts_with(&canonical_base) {
                log::warn!("Import: path escapes data dir: {:?}", canonical);
                continue;
            }

            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            std::fs::write(&out_path, &buf)
                .map_err(|e| format!("Failed to write {}: {}", name, e))?;
        }

        Ok(())
    }).await.map_err(|e| e.to_string())??;

    log::info!("Imported backup from zip");

    // Rebuild in-memory caches from the imported DB
    crate::clipboard::load_search_cache(&db.pool).await;
    crate::clipboard::load_settings_cache(&db.pool).await;

    // Notify frontend to restart
    let _ = app.emit("data-directory-changed", &serde_json::json!({
        "message": "Backup imported. Please restart the application.",
        "new_path": data_dir.to_string_lossy()
    }));

    Ok(())
}

#[tauri::command]
pub async fn get_dashboard_stats(db: tauri::State<'_, Arc<Database>>) -> Result<serde_json::Value, String> {
    let pool = &db.pool;

    // Consolidate 4 count queries into 1
    let (total, today, images, folders): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM clips) as total,
            (SELECT COUNT(*) FROM clips WHERE date(created_at,'localtime') = date('now','localtime')) as today,
            (SELECT COUNT(*) FROM clips WHERE clip_type = 'image') as images,
            (SELECT COUNT(*) FROM folders) as folders"
    ).fetch_one(pool).await.map_err(|e| e.to_string())?;

    // Clips per day (last 7 days)
    let daily: Vec<(String, i64)> = sqlx::query_as(
        "SELECT date(created_at, 'localtime') as day, COUNT(*) as count
         FROM clips WHERE date(created_at, 'localtime') >= date('now', 'localtime', '-6 days')
         GROUP BY date(created_at, 'localtime') ORDER BY day ASC"
    ).fetch_all(pool).await.map_err(|e| e.to_string())?;

    // Top source apps (top 5)
    let top_apps: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(source_app, 'Unknown') as app, COUNT(*) as count
         FROM clips WHERE source_app IS NOT NULL
         GROUP BY source_app ORDER BY count DESC LIMIT 5"
    ).fetch_all(pool).await.map_err(|e| e.to_string())?;

    // Most pasted clips (top 5)
    let most_pasted: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT uuid, SUBSTR(text_preview, 1, 80), paste_count
         FROM clips WHERE paste_count > 0
         ORDER BY paste_count DESC LIMIT 5"
    ).fetch_all(pool).await.map_err(|e| e.to_string())?;

    // DB file size
    let db_path = db.images_dir.parent().unwrap().join("clipboard.db");
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    // Images dir size
    let mut images_size: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(&db.images_dir) {
        for entry in entries.flatten() {
            images_size += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }

    Ok(serde_json::json!({
        "total": total,
        "today": today,
        "images": images,
        "folders": folders,
        "daily": daily.iter().map(|(day, count)| {
            serde_json::json!({ "day": day, "count": count })
        }).collect::<Vec<_>>(),
        "top_apps": top_apps.iter().map(|(app, count)| {
            serde_json::json!({ "app": app, "count": count })
        }).collect::<Vec<_>>(),
        "most_pasted": most_pasted.iter().map(|(uuid, preview, count)| {
            serde_json::json!({ "id": uuid, "preview": preview, "count": count })
        }).collect::<Vec<_>>(),
        "db_size": db_size,
        "images_size": images_size,
    }))
}

#[tauri::command]
pub async fn get_clips_by_date(date: String, search: Option<String>, source_app: Option<String>, db: tauri::State<'_, Arc<Database>>) -> Result<Vec<ClipboardItem>, String> {
    let pool = &db.pool;

    let has_search = search.as_ref().map_or(false, |s| !s.is_empty());
    let has_app = source_app.as_ref().map_or(false, |s| !s.is_empty());

    let mut sql = String::from(
        "SELECT id, uuid, clip_type,
                CASE WHEN clip_type = 'image' THEN content ELSE '' END as content,
                text_preview, content_hash,
                folder_id, is_deleted, source_app, source_icon, metadata,
                created_at, last_accessed, last_pasted_at, is_pinned,
                subtype, note, paste_count
         FROM clips WHERE date(created_at, 'localtime') = ?"
    );
    if has_search { sql.push_str(" AND text_preview LIKE ?"); }
    if has_app { sql.push_str(" AND source_app = ?"); }
    sql.push_str(" ORDER BY created_at DESC LIMIT 100");

    let mut query = sqlx::query_as::<_, Clip>(&sql).bind(&date);
    if has_search { query = query.bind(format!("%{}%", search.as_ref().unwrap())); }
    if has_app { query = query.bind(source_app.as_ref().unwrap()); }

    let clips: Vec<Clip> = query.fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut items = Vec::with_capacity(clips.len());
    for clip in &clips {
        items.push(clip_to_item_async(clip, &db.images_dir, false).await);
    }

    Ok(items)
}

/// Get list of dates that have clips (for calendar highlighting)
#[tauri::command]
pub async fn get_clip_dates(db: tauri::State<'_, Arc<Database>>) -> Result<Vec<serde_json::Value>, String> {
    let pool = &db.pool;
    let dates: Vec<(String, i64)> = sqlx::query_as(
        "SELECT date(created_at, 'localtime') as day, COUNT(*) as count FROM clips
         GROUP BY date(created_at, 'localtime') ORDER BY day DESC LIMIT 365"
    ).fetch_all(pool).await.map_err(|e| e.to_string())?;

    Ok(dates.iter().map(|(day, count)| {
        serde_json::json!({ "date": day, "count": count })
    }).collect())
}
