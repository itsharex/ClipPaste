use tauri::AppHandle;
use tauri_plugin_clipboard_x::{write_text, stop_listening, start_listening};
use crate::models::{Clip, ClipboardItem};
use std::path::Path;

/// Convert a Clip DB row to a ClipboardItem for the frontend.
/// For images: returns the absolute file path (frontend uses convertFileSrc()).
/// For text: returns the text_preview.
pub async fn clip_to_item_async(clip: &Clip, images_dir: &Path, preview_only: bool) -> ClipboardItem {
    let content_str = if preview_only && clip.clip_type == "image" {
        String::new()
    } else if clip.clip_type == "image" {
        let filename = String::from_utf8_lossy(&clip.content).to_string();
        let image_path = images_dir.join(&filename);
        image_path.to_string_lossy().to_string()
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
        is_sensitive: clip.is_sensitive,
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

/// Set clipboard hashes without writing (for image clips handled by frontend).
pub async fn clipboard_set_hashes(content_hash: &str) {
    crate::clipboard::set_ignore_hash(content_hash.to_string());
    crate::clipboard::set_last_stable_hash(content_hash.to_string());
}

/// Check auto_paste setting and hide the window accordingly.
pub fn check_auto_paste_and_hide(window: &tauri::WebviewWindow) {
    let auto_paste = crate::clipboard::get_cached_setting("auto_paste")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    if auto_paste {
        let window_clone = window.clone();
        crate::animate_window_hide(window, Some(Box::new(move || {
            #[cfg(target_os = "windows")]
            {
                std::thread::sleep(std::time::Duration::from_millis(200));
                crate::clipboard::send_paste_input();
            }
            let _ = &window_clone; // suppress unused warning on non-Windows
        })));
    } else {
        crate::animate_window_hide(window, None);
    }
}
