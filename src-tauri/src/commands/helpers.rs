use tauri::AppHandle;
use tauri_plugin_clipboard_x::{write_text, write_image, stop_listening, start_listening};
use crate::models::{Clip, ClipboardItem};
use std::path::Path;

/// Convert a Clip DB row to a ClipboardItem for the frontend.
/// For images: returns the absolute file path (frontend uses convertFileSrc()).
/// For text: returns the text_preview.
pub async fn clip_to_item_async(clip: &Clip, images_dir: &Path, preview_only: bool) -> ClipboardItem {
    let content_str = if preview_only && clip.clip_type == "image" {
        String::new()
    } else if clip.clip_type == "image" {
        let filename = String::from_utf8_lossy(&clip.content).into_owned();
        let image_path = images_dir.join(&filename);
        image_path.to_string_lossy().to_string()
    } else {
        clip.text_preview.clone()
    };

    // For images, check if a thumbnail exists ({hash}_thumb.jpg)
    let thumbnail = if clip.clip_type == "image" {
        let filename = String::from_utf8_lossy(&clip.content).into_owned();
        let hash = filename.trim_end_matches(".png");
        let thumb_filename = format!("{}_thumb.jpg", hash);
        let thumb_path = images_dir.join(&thumb_filename);
        if thumb_path.exists() {
            Some(thumb_path.to_string_lossy().to_string())
        } else {
            None
        }
    } else {
        None
    };

    ClipboardItem {
        id: clip.uuid.clone(),
        clip_type: clip.clip_type.clone(),
        content: content_str,
        preview: clip.text_preview.clone(),
        folder_id: clip.folder_id.map(|id| id.to_string()),
        created_at: clip.created_at.to_rfc3339(),
        source_app: clip.source_app.clone(),
        source_icon: clip.source_icon.clone().or_else(|| {
            clip.source_app.as_ref().and_then(|app| crate::clipboard::get_app_icon(app))
        }),
        metadata: clip.metadata.clone(),
        is_pinned: clip.is_pinned,
        subtype: clip.subtype.clone(),
        note: clip.note.clone(),
        paste_count: clip.paste_count,
        is_sensitive: clip.is_sensitive,
        thumbnail,
    }
}

/// Write text to clipboard with retry logic, managing listener stop/start.
/// Returns Ok(()) on success, Err with message on failure.
pub async fn clipboard_write_text(app: &AppHandle, text: &str, content_hash: &str) -> Result<(), String> {
    let _guard = crate::clipboard::CLIPBOARD_SYNC.lock().await;

    crate::clipboard::set_ignore_hash(content_hash.to_string());
    crate::clipboard::set_last_stable_hash(content_hash.to_string());

    if let Err(e) = stop_listening().await {
        log::error!("Failed to stop listener: {}", e);
    }

    let mut last_err = String::new();
    for i in 0..5 {
        match write_text(text.to_string()).await {
            Ok(_) => { last_err.clear(); break; },
            Err(e) => {
                last_err = e.to_string();
                log::warn!("Clipboard write attempt {} failed: {}. Retrying...", i + 1, last_err);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    if let Err(e) = start_listening(app.clone()).await {
        log::error!("Failed to restart listener: {}", e);
    }

    if !last_err.is_empty() {
        return Err(format!("Failed to write clipboard text: {}", last_err));
    }

    Ok(())
}

/// Write image to clipboard with retry logic, managing listener stop/start.
/// `image_path` is the absolute path to the image file on disk.
pub async fn clipboard_write_image(app: &AppHandle, image_path: &str, content_hash: &str) -> Result<(), String> {
    let _guard = crate::clipboard::CLIPBOARD_SYNC.lock().await;

    crate::clipboard::set_ignore_hash(content_hash.to_string());
    crate::clipboard::set_last_stable_hash(content_hash.to_string());

    if let Err(e) = stop_listening().await {
        log::error!("Failed to stop listener: {}", e);
    }

    let mut last_err = String::new();
    for i in 0..5 {
        match write_image(image_path.to_string()).await {
            Ok(_) => { last_err.clear(); break; },
            Err(e) => {
                last_err = e.to_string();
                log::warn!("Clipboard image write attempt {} failed: {}. Retrying...", i + 1, last_err);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    if let Err(e) = start_listening(app.clone()).await {
        log::error!("Failed to restart listener: {}", e);
    }

    if !last_err.is_empty() {
        return Err(format!("Failed to write clipboard image: {}", last_err));
    }

    Ok(())
}

/// Check auto_paste setting and hide the window accordingly.
pub fn check_auto_paste_and_hide(window: &tauri::WebviewWindow) {
    use tauri::Manager;
    let auto_paste = crate::clipboard::get_cached_setting("auto_paste")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    if auto_paste {
        let window_clone = window.clone();
        let db = window.app_handle().state::<std::sync::Arc<crate::database::Database>>().inner().clone();
        crate::animate_window_hide(window, Some(Box::new(move || {
            #[cfg(target_os = "windows")]
            {
                std::thread::sleep(std::time::Duration::from_millis(200));
                // After hide+sleep the foreground window IS the paste target.
                // Skip the keystroke if the target app is in the Ignored list.
                if crate::clipboard::is_foreground_app_ignored(&db) {
                    log::info!("PASTE: Suppressed Shift+Insert (target app is ignored)");
                } else {
                    crate::clipboard::send_paste_input();
                }
            }
            let _ = &window_clone; // suppress unused warning on non-Windows
            let _ = &db;
        })));
    } else {
        crate::animate_window_hide(window, None);
    }
}
