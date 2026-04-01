use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::models::{Clip, ClipboardItem};
use std::path::Path;

/// Convert a Clip DB row to a ClipboardItem for the frontend.
/// When `preview_only` is true, image content is omitted (empty string).
/// Uses async I/O so it doesn't block the Tokio runtime when reading image files.
pub async fn clip_to_item_async(clip: &Clip, images_dir: &Path, preview_only: bool) -> ClipboardItem {
    let content_str = if preview_only && clip.clip_type == "image" {
        String::new()
    } else if clip.clip_type == "image" {
        let filename = String::from_utf8_lossy(&clip.content).to_string();
        let image_path = images_dir.join(&filename);
        match tokio::fs::read(&image_path).await {
            Ok(bytes) => BASE64.encode(&bytes),
            Err(_) => String::new(),
        }
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
