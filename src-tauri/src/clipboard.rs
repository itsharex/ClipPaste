
use tauri::{AppHandle, Listener, Emitter};
// Import functions directly from the crate root
use tauri_plugin_clipboard_x::{read_image, read_text, start_listening};
use std::sync::Arc;
use crate::database::Database;
use uuid::Uuid;
use sha2::{Digest, Sha256};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::MAX_PATH;
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
#[cfg(target_os = "windows")]
use windows::Win32::System::ProcessStatus::{GetModuleBaseNameW, GetModuleFileNameExW};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
#[cfg(target_os = "windows")]
use windows::Win32::System::DataExchange::GetClipboardOwner;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, DestroyIcon, DrawIconEx, DI_NORMAL, GetIconInfo, ICONINFO};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_SHIFT, VK_INSERT};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHGFI_ICON, SHGFI_LARGEICON, SHFILEINFOW, SHGFI_USEFILEATTRIBUTES};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    GetObjectW, GetDC, ReleaseDC, CreateCompatibleDC, SelectObject, DeleteDC,
    GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    BITMAP, HBITMAP, CreateCompatibleBitmap, DeleteObject
};
#[cfg(target_os = "windows")]
use std::ffi::OsStr;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
use once_cell::sync::Lazy;

// GLOBAL STATE: Combined clipboard hash tracking under a single lock to prevent race conditions.
// - ignore_hash: hash of the clip we just pasted ourselves (skip re-capture)
// - last_stable_hash: hash of the last processed clipboard content (dedup)
struct ClipboardHashState {
    ignore_hash: Option<String>,
    last_stable_hash: Option<String>,
}
static HASH_STATE: Lazy<parking_lot::Mutex<ClipboardHashState>> = Lazy::new(|| {
    parking_lot::Mutex::new(ClipboardHashState {
        ignore_hash: None,
        last_stable_hash: None,
    })
});
pub static CLIPBOARD_SYNC: Lazy<Arc<tokio::sync::Mutex<()>>> = Lazy::new(|| Arc::new(tokio::sync::Mutex::new(())));

/// In-memory search index: uuid → (preview_lowercase, folder_id, note_lowercase)
/// Loaded once at startup, updated on each clipboard change. HashMap for O(1) remove/update.
type SearchCacheMap = std::collections::HashMap<String, (String, Option<i64>, String)>;
pub static SEARCH_CACHE: Lazy<parking_lot::RwLock<SearchCacheMap>> =
    Lazy::new(|| parking_lot::RwLock::new(std::collections::HashMap::new()));

/// In-memory settings cache: avoids DB round-trips for hot-path settings like auto_paste, ignore_ghost_clips.
pub static SETTINGS_CACHE: Lazy<parking_lot::RwLock<std::collections::HashMap<String, String>>> =
    Lazy::new(|| parking_lot::RwLock::new(std::collections::HashMap::new()));

#[cfg(target_os = "windows")]
pub static ICON_CACHE: Lazy<parking_lot::Mutex<lru::LruCache<String, Option<String>>>> =
    Lazy::new(|| parking_lot::Mutex::new(lru::LruCache::new(std::num::NonZeroUsize::new(100).unwrap())));

/// App icon lookup: app_name → base64 icon. New clips use this instead of per-clip source_icon.
pub static APP_ICONS_CACHE: Lazy<parking_lot::RwLock<std::collections::HashMap<String, String>>> =
    Lazy::new(|| parking_lot::RwLock::new(std::collections::HashMap::new()));

/// Incognito mode: when true, clipboard changes are not captured
pub static IS_INCOGNITO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Snapshot of the foreground HWND taken before a helper window (scratchpad) steals focus.
/// Used by `scratchpad_paste` to route Shift+Insert back to the user's target app.
/// Stored as isize because raw pointers aren't Send, but HWND is just an opaque handle.
#[cfg(target_os = "windows")]
pub static PREV_FOREGROUND_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

use std::sync::atomic::{AtomicU64, Ordering};
static DEBOUNCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Maximum entries in the search cache. Clips beyond this are still in DB but not instant-searchable.
const SEARCH_CACHE_MAX: usize = 50_000;

/// Version of the sensitive/subtype detection rules. Bump this when `detect_sensitive`
/// or `detect_subtype` logic changes in a way that should re-classify existing clips.
/// On startup, if the DB-stored version is lower, rescan runs; otherwise it's skipped.
pub const DETECTION_RULES_VERSION: i64 = 1;

/// Load all clip previews into memory for instant search.
/// Capped at SEARCH_CACHE_MAX entries (most recent first) to bound memory usage.
pub async fn load_search_cache(pool: &sqlx::SqlitePool) {
    let result = sqlx::query_as::<_, (String, String, Option<i64>, Option<String>)>(
        "SELECT uuid, COALESCE(text_preview, ''), folder_id, note FROM clips ORDER BY created_at DESC LIMIT ?"
    ).bind(SEARCH_CACHE_MAX as i64).fetch_all(pool).await;
    let rows = match result {
        Ok(r) => r,
        Err(e) => {
            log::error!("SEARCH_CACHE: Failed to load: {}", e);
            Vec::new()
        }
    };

    let mut map = std::collections::HashMap::with_capacity(rows.len());
    for (uuid, preview, fid, note) in rows {
        map.insert(uuid, (preview.to_lowercase(), fid, note.unwrap_or_default().to_lowercase()));
    }

    let count = map.len();
    *SEARCH_CACHE.write() = map;
    log::info!("SEARCH_CACHE: Loaded {} clip previews into memory (max {})", count, SEARCH_CACHE_MAX);
}

/// Add a single clip to the search cache
pub fn add_to_search_cache(uuid: &str, preview: &str, folder_id: Option<i64>) {
    let mut cache = SEARCH_CACHE.write();
    cache.insert(uuid.to_string(), (preview.to_lowercase(), folder_id, String::new()));
}

/// Remove a clip from the search cache — O(1) with HashMap
pub fn remove_from_search_cache(uuid: &str) {
    let mut cache = SEARCH_CACHE.write();
    cache.remove(uuid);
}

/// Re-sync a single clip's entry in the search cache from DB.
/// Used by the re-copy self-heal path: if a clip's cache entry was missing or stale,
/// re-copying the same content forces it back into the cache with current folder_id + note.
pub async fn refresh_search_cache_for_clip(pool: &sqlx::SqlitePool, uuid: &str, preview: &str) {
    let row: Option<(Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT folder_id, note FROM clips WHERE uuid = ?"
    ).bind(uuid).fetch_optional(pool).await.unwrap_or(None);
    let (fid, note) = row.unwrap_or((None, None));
    let mut cache = SEARCH_CACHE.write();
    cache.insert(
        uuid.to_string(),
        (preview.to_lowercase(), fid, note.unwrap_or_default().to_lowercase()),
    );
}

/// Update a clip's note in the search cache — O(1) with HashMap
pub fn update_note_in_search_cache(uuid: &str, note: Option<&str>) {
    let mut cache = SEARCH_CACHE.write();
    if let Some(entry) = cache.get_mut(uuid) {
        entry.2 = note.unwrap_or_default().to_lowercase();
    }
}

/// Load all settings into memory for instant access
pub async fn load_settings_cache(pool: &sqlx::SqlitePool) {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(pool).await.unwrap_or_default();
    let mut cache = SETTINGS_CACHE.write();
    cache.clear();
    for (key, value) in rows {
        cache.insert(key, value);
    }
    log::info!("SETTINGS_CACHE: Loaded {} settings into memory", cache.len());
}

/// Get a setting from the in-memory cache (no DB round-trip)
pub fn get_cached_setting(key: &str) -> Option<String> {
    SETTINGS_CACHE.read().get(key).cloned()
}

/// Load all app icons into memory for instant lookup
pub async fn load_app_icons_cache(pool: &sqlx::SqlitePool) {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT app_name, icon FROM app_icons")
        .fetch_all(pool).await.unwrap_or_default();
    let count = rows.len();
    let mut cache = APP_ICONS_CACHE.write();
    cache.clear();
    for (name, icon) in rows {
        cache.insert(name, icon);
    }
    log::info!("APP_ICONS_CACHE: Loaded {} app icons into memory", count);
}

/// Get an app icon from the in-memory cache
pub fn get_app_icon(app_name: &str) -> Option<String> {
    APP_ICONS_CACHE.read().get(app_name).cloned()
}

/// Save app icon to DB + cache (deduplicated per app_name)
async fn save_app_icon(pool: &sqlx::SqlitePool, app_name: &str, icon: &str) {
    APP_ICONS_CACHE.write().insert(app_name.to_string(), icon.to_string());
    sqlx::query("INSERT OR REPLACE INTO app_icons (app_name, icon) VALUES (?, ?)")
        .bind(app_name).bind(icon).execute(pool).await.ok();
}

pub fn truncate_utf8(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Detect content subtype from text (url, email, color, path)
pub fn detect_subtype(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() { return None; }

    // URL
    if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
        && !trimmed.contains(char::is_whitespace)
    {
        return Some("url".to_string());
    }

    // Email (simple check: single @, no spaces, domain has dot)
    if trimmed.contains('@') && !trimmed.contains(char::is_whitespace) {
        let parts: Vec<&str> = trimmed.split('@').collect();
        if parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.') {
            return Some("email".to_string());
        }
    }

    // Color: hex (#fff, #ffffff, #ffffffff)
    if let Some(hex) = trimmed.strip_prefix('#') {
        if (hex.len() == 3 || hex.len() == 6 || hex.len() == 8)
            && hex.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Some("color".to_string());
        }
    }

    // Color: rgb()/rgba()/hsl()/hsla()
    if trimmed.starts_with("rgb(") || trimmed.starts_with("rgba(")
        || trimmed.starts_with("hsl(") || trimmed.starts_with("hsla(")
    {
        return Some("color".to_string());
    }

    // File path: Windows (C:\...) or UNC (\\...)
    if trimmed.len() >= 3 {
        let bytes = trimmed.as_bytes();
        if bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
            return Some("path".to_string());
        }
    }

    // File path: Unix absolute
    if trimmed.starts_with('/') && !trimmed.contains(char::is_whitespace) && trimmed.len() > 1 {
        return Some("path".to_string());
    }

    // Phone number: +1-234-567-8900, (234) 567-8900, etc. (single-line, short)
    if trimmed.len() <= 25 && !trimmed.contains(char::is_alphabetic) {
        let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 7 && digits.len() <= 15
            && trimmed.chars().all(|c| c.is_ascii_digit() || "+-() .,#".contains(c))
        {
            return Some("phone".to_string());
        }
    }

    // JSON: starts with { or [ and parses as valid JSON (min length 5 to skip trivial cases)
    if trimmed.len() >= 5 && (trimmed.starts_with('{') || trimmed.starts_with('[')) {
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return Some("json".to_string());
        }
    }

    // Code: multi-line text with programming patterns (need ≥2 indicators)
    if trimmed.lines().count() >= 3 {
        let has_braces = trimmed.contains('{') && trimmed.contains('}');
        let has_semicolons = trimmed.matches(';').count() >= 2;
        let has_fn_keyword = ["function ", "fn ", "def ", "class ", "const ", "let ",
            "import ", "pub ", "#include", "package ", "var ", "return ", "async "]
            .iter().any(|kw| trimmed.contains(kw));
        let has_indentation = trimmed.lines().filter(|l| l.starts_with("    ") || l.starts_with('\t')).count() >= 2;
        let has_arrows_or_ops = trimmed.contains("=>") || trimmed.contains("->") || trimmed.contains("::");
        let indicators = [has_braces, has_semicolons, has_fn_keyword, has_indentation, has_arrows_or_ops]
            .iter().filter(|&&x| x).count();
        if indicators >= 2 {
            return Some("code".to_string());
        }
    }

    None
}

/// Maximum thumbnail width in pixels. Cards are ~210px wide, so 280px gives good quality at 1.3x.
const THUMBNAIL_MAX_WIDTH: u32 = 280;

/// Generate a JPEG thumbnail from PNG image bytes, resized to fit within max_width.
/// Returns the thumbnail bytes or None on failure.
pub fn generate_thumbnail(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(png_bytes).ok()?;
    let (w, _h) = (img.width(), img.height());
    let thumb = if w > THUMBNAIL_MAX_WIDTH {
        img.thumbnail(THUMBNAIL_MAX_WIDTH, u32::MAX)
    } else {
        img
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut buf, image::ImageOutputFormat::Jpeg(80)).ok()?;
    Some(buf.into_inner())
}

pub fn set_ignore_hash(hash: String) {
    let mut state = HASH_STATE.lock();
    state.ignore_hash = Some(hash);
}

pub fn set_last_stable_hash(hash: String) {
    let mut state = HASH_STATE.lock();
    state.last_stable_hash = Some(hash);
}

pub fn init(app: &AppHandle, db: Arc<Database>) {
    let app_clone = app.clone();
    let db_clone = db.clone();

    // Start monitor
    // tauri-plugin-clipboard-x exposes start_listening(app_handle)
    // It returns impl Future, so we need to spawn it or block.
    // Since init is synchronous here, we spawn it.
    let app_for_start = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = start_listening(app_for_start).await {
            log::error!("CLIPBOARD: Failed to start listener: {}", e);
        }
    });

    // Listen to clipboard changes
    // The event name found in source code: "plugin:clipboard-x://clipboard_changed"
    let event_name = "plugin:clipboard-x://clipboard_changed";

    app.listen(event_name, move |_event| {
        let app = app_clone.clone();
        let db = db_clone.clone();

        // Capture source app info IMMEDIATELY before debounce.
        // On macOS, frontmostApplication returns the current foreground app,
        // which may change during the 150ms debounce window.
        let source_app_info = get_clipboard_owner_app_info();

        // DEBOUNCE LOGIC:
        let current_count = DEBOUNCE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

        tauri::async_runtime::spawn(async move {
            let debounce_ms = get_cached_setting("debounce_ms")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(150);
            tokio::time::sleep(std::time::Duration::from_millis(debounce_ms)).await;

            if DEBOUNCE_COUNTER.load(Ordering::SeqCst) != current_count {
                log::debug!("CLIPBOARD: Debounce: Aborting older event, current_count:{}", current_count);
                return;
            }

            process_clipboard_change(app, db, source_app_info).await;
        });
    });
}

type SourceAppInfo = (Option<String>, Option<String>, Option<String>, Option<String>, bool);

/// Detect if text content contains sensitive information (API keys, passwords, credit cards, etc.)
pub fn detect_sensitive(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() { return None; }

    // AWS access key
    if trimmed.contains("AKIA") && trimmed.len() >= 20 {
        return Some("aws_key".to_string());
    }
    // GitHub personal access token
    if trimmed.starts_with("ghp_") || trimmed.starts_with("gho_") || trimmed.starts_with("ghs_") {
        return Some("github_token".to_string());
    }
    // Stripe secret key
    if trimmed.starts_with("sk_live_") || trimmed.starts_with("sk_test_") {
        return Some("stripe_key".to_string());
    }
    // Slack tokens
    if trimmed.starts_with("xoxb-") || trimmed.starts_with("xoxp-") || trimmed.starts_with("xoxa-") {
        return Some("slack_token".to_string());
    }
    // Private keys
    if trimmed.contains("-----BEGIN") && trimmed.contains("PRIVATE KEY-----") {
        return Some("private_key".to_string());
    }
    // JWT tokens (3 base64 segments separated by dots)
    if trimmed.starts_with("eyJ") && trimmed.matches('.').count() == 2
        && !trimmed.contains(char::is_whitespace)
    {
        return Some("jwt".to_string());
    }
    // Credit card numbers (13-19 digits, possibly with spaces/dashes)
    let digits_only: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits_only.len() >= 13 && digits_only.len() <= 19
        && trimmed.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '-')
        && luhn_check(&digits_only)
    {
        return Some("credit_card".to_string());
    }

    // Password-like strings: high-entropy random strings that look like secrets.
    // Must pass ALL checks to reduce false positives on normal text like domain names,
    // container names, file paths, emails, etc.
    if trimmed.len() >= 8 && trimmed.len() <= 64 && !trimmed.contains(char::is_whitespace) {
        // Skip common non-password patterns
        let dominated_by_separators = trimmed.starts_with("http://") || trimmed.starts_with("https://")
            || trimmed.contains('@')        // emails
            || trimmed.contains("://")      // URIs
            || trimmed.contains('/')        // paths
            || trimmed.contains('\\')       // Windows paths
            || trimmed.contains('=')        // env vars (KEY=value)
            || trimmed.starts_with(':')     // config tokens (:_authToken)
            || trimmed.starts_with('.')     // dotfiles
            || trimmed.ends_with(".com") || trimmed.ends_with(".net") || trimmed.ends_with(".org")
            || trimmed.ends_with(".io") || trimmed.ends_with(".log") || trimmed.ends_with(".conf")
            || trimmed.contains(".log");     // log filenames

        if !dominated_by_separators {
            let has_upper = trimmed.chars().any(|c| c.is_ascii_uppercase());
            let has_lower = trimmed.chars().any(|c| c.is_ascii_lowercase());
            let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
            let has_special = trimmed.chars().any(|c| !c.is_alphanumeric() && c.is_ascii());
            let classes = [has_upper, has_lower, has_digit, has_special].iter().filter(|&&x| x).count();

            // Require ALL 4 character classes, or 3 classes with min length 12 for high-entropy strings
            // This filters out things like "worker-1" (too short) and "camel-service-worker-2" (too long/readable)
            if classes == 4 || (classes >= 3 && trimmed.len() >= 12 && trimmed.len() <= 20) {
                // Final check: must have enough "randomness"
                // 1) At least 25% non-lowercase chars (filters out "worker-1" style names)
                let non_lower = trimmed.chars().filter(|c| !c.is_ascii_lowercase()).count();
                if non_lower * 4 < trimmed.len() {
                    // Too few non-lowercase chars — looks like a readable name, not a password
                } else {
                    // 2) Must not be a simple word-separator-word pattern (e.g. "Service-Name1")
                    let separator_count = trimmed.chars().filter(|c| *c == '-' || *c == '_' || *c == '.').count();
                    if separator_count <= 1 || trimmed.len() > 20 {
                        return Some("password".to_string());
                    }
                }
            }
        }
    }

    None
}

/// Luhn algorithm for credit card validation
fn luhn_check(digits: &str) -> bool {
    let mut sum = 0u32;
    let mut double = false;
    for c in digits.chars().rev() {
        if let Some(d) = c.to_digit(10) {
            let mut val = if double { d * 2 } else { d };
            if val > 9 { val -= 9; }
            sum += val;
            double = !double;
        } else {
            return false;
        }
    }
    sum.is_multiple_of(10)
}

async fn process_clipboard_change(app: AppHandle, db: Arc<Database>, source_app_info: SourceAppInfo) {
    let _guard = CLIPBOARD_SYNC.lock().await;

    // Check incognito mode AFTER acquiring lock to prevent race with toggle during debounce
    if IS_INCOGNITO.load(Ordering::SeqCst) {
        log::debug!("CLIPBOARD: Incognito mode active, skipping capture");
        return;
    }

    let mut clip_type = "text";
    let mut clip_content = Vec::new();
    let mut clip_preview = String::new();
    let mut clip_hash = String::new();
    let mut metadata = String::new();
    let mut clip_subtype: Option<String> = None;
    let mut found_content = false;

    // Try Image
    if let Ok(read_image_result) = read_image(app.clone(), None).await {
         if let Ok(bytes) = std::fs::read(&read_image_result.path) {
             if let Ok(reader) = image::io::Reader::new(std::io::Cursor::new(&bytes)).with_guessed_format() {
               if let Ok((width, height)) = reader.into_dimensions() {
                 let size_bytes = bytes.len();

                 clip_hash = calculate_hash(&bytes);

                 // Save image to disk: {images_dir}/{hash}.png
                 let filename = format!("{}.png", &clip_hash);
                 let image_file_path = db.image_path(&filename);
                 if !image_file_path.exists() {
                     if let Err(e) = std::fs::write(&image_file_path, &bytes) {
                         log::error!("CLIPBOARD: Failed to save image to disk: {}", e);
                         let _ = std::fs::remove_file(read_image_result.path);
                         return;
                     }
                 }

                 // Generate and save thumbnail as {hash}_thumb.jpg
                 let thumb_filename = format!("{}_thumb.jpg", &clip_hash);
                 let thumb_path = db.image_path(&thumb_filename);
                 if !thumb_path.exists() {
                     if let Some(thumb_bytes) = generate_thumbnail(&bytes) {
                         if let Err(e) = std::fs::write(&thumb_path, &thumb_bytes) {
                             log::warn!("CLIPBOARD: Failed to save thumbnail: {}", e);
                         }
                     }
                 }

                 // Store just the filename in content (not the raw blob)
                 clip_content = filename.as_bytes().to_vec();
                 clip_type = "image";
                 clip_preview = "[Image]".to_string();
                 metadata = serde_json::json!({
                     "width": width,
                     "height": height,
                     "format": "png",
                     "size_bytes": size_bytes
                 }).to_string();
                 found_content = true;

                 // Clean up the temp file from clipboard plugin
                 let _ = std::fs::remove_file(read_image_result.path);
               }
             }
         }
    }

    if !found_content {
        // Try Text
        if let Ok(text) = read_text().await {
             let text = text.trim();
             if !text.is_empty() {
                 clip_content = text.as_bytes().to_vec();
                 clip_hash = calculate_hash(&clip_content);
                 clip_type = "text";
                 clip_preview = truncate_utf8(text, 2000).to_string();
                 clip_subtype = detect_subtype(text);
                 found_content = true;
                 log::trace!("CLIPBOARD: Found text ({} chars, subtype: {:?})", clip_preview.len(), clip_subtype);
             }
        }
    }

    if !found_content {
        return;
    }

    // Atomic hash check: dedup + self-paste detection under single lock
    {
        let mut state = HASH_STATE.lock();
        if let Some(ref last_hash) = state.last_stable_hash {
            if last_hash == &clip_hash {
                return;
            }
        }
        state.last_stable_hash = Some(clip_hash.clone());
        if let Some(ignore_hash) = state.ignore_hash.take() {
            if ignore_hash == clip_hash {
                log::info!("CLIPBOARD: Detected self-paste, proceeding to update timestamp");
            }
        }
    }

    // Source app info was captured before debounce to ensure accuracy on macOS
    let (source_app, source_icon, exe_name, full_path, is_explicit_owner) = source_app_info;
    log::debug!("CLIPBOARD: Source app: {:?}, explicit: {}", source_app, is_explicit_owner);

    // Check ignore_ghost_clips setting (from in-memory cache, no DB round-trip)
    let ignore_ghost_clips = get_cached_setting("ignore_ghost_clips")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if ignore_ghost_clips && !is_explicit_owner {
        log::info!("CLIPBOARD: Ignoring ghost clip (unknown owner)");
        return;
    }

    // Check if the app is in the ignore list
    if let Some(ref path) = full_path {
        if let Ok(true) = db.is_app_ignored(path).await {
             log::info!("CLIPBOARD: Ignoring content from ignored app (path match): {}", path);
             return;
        }
    }

    if let Some(ref exe) = exe_name {
        if let Ok(true) = db.is_app_ignored(exe).await {
             log::info!("CLIPBOARD: Ignoring content from ignored app (exe match): {}", exe);
             return;
        }
    }

    // DB Logic
    let pool = &db.pool;

    let existing_uuid: Option<String> = sqlx::query_scalar::<_, String>(r#"SELECT uuid FROM clips WHERE content_hash = ?"#)
        .bind(&clip_hash)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    if let Some(existing_id) = existing_uuid {
        // Bump created_at + re-evaluate is_sensitive (detection rules may have changed)
        let is_sensitive = if clip_type == "text" {
            detect_sensitive(&clip_preview).is_some()
        } else {
            false
        };
        if let Err(e) = sqlx::query(r#"UPDATE clips SET created_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, is_sensitive = ? WHERE uuid = ?"#)
            .bind(is_sensitive)
            .bind(&existing_id)
            .execute(pool)
            .await
        {
            log::error!("CLIPBOARD: Failed to update existing clip: {}", e);
            return;
        }

        // Self-heal: re-copying a clip should make it searchable again even if
        // it was missing from the in-memory cache for any reason.
        refresh_search_cache_for_clip(pool, &existing_id, &clip_preview).await;

        let _ = app.emit("clipboard-change", &serde_json::json!({
            "id": existing_id,
            "content": clip_preview,
            "clip_type": clip_type,
            "source_app": source_app,
            "source_icon": source_icon,
            "created_at": chrono::Utc::now().to_rfc3339()
        }));
    } else {
        let clip_uuid = Uuid::new_v4().to_string();

        // Detect sensitive content in text clips
        let is_sensitive = if clip_type == "text" {
            detect_sensitive(&clip_preview).is_some()
        } else {
            false
        };

        // Save icon to app_icons lookup (deduplicated per app)
        if let (Some(ref app_name), Some(ref icon)) = (&source_app, &source_icon) {
            if !icon.is_empty() {
                save_app_icon(pool, app_name, icon).await;
            }
        }

        if let Err(e) = sqlx::query(r#"
            INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, folder_id, is_deleted, source_app, source_icon, metadata, subtype, is_sensitive, created_at, last_accessed, updated_at)
            VALUES (?, ?, ?, ?, ?, NULL, 0, ?, NULL, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#)
        .bind(&clip_uuid)
        .bind(clip_type)
        .bind(&clip_content)
        .bind(&clip_preview)
        .bind(&clip_hash)
        .bind(&source_app)
        .bind(if clip_type == "image" { Some(metadata) } else { None })
        .bind(&clip_subtype)
        .bind(is_sensitive)
        .execute(pool)
        .await
        {
            log::error!("CLIPBOARD: Failed to insert new clip: {}", e);
            return;
        }

        // Update in-memory search cache
        add_to_search_cache(&clip_uuid, &clip_preview, None);

        // FTS5 index no longer used — search uses in-memory SEARCH_CACHE

        let _ = app.emit("clipboard-change", &serde_json::json!({
            "id": clip_uuid,
            "content": clip_preview,
            "clip_type": clip_type,
            "source_app": source_app,
            "source_icon": source_icon,
            "created_at": chrono::Utc::now().to_rfc3339()
        }));

        // Enforce limits after each insert (cache check is O(1), returns early if not configured)
        if get_cached_setting("max_items").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0) > 0 {
            db.enforce_max_items().await;
        }
        if get_cached_setting("auto_delete_days").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0) > 0 {
            db.enforce_auto_delete().await;
        }
    }
}

pub fn calculate_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    format!("{:x}", result)
}

// ========== PLATFORM-SPECIFIC: Source app detection ==========

/// Read the current foreground window's app info (name + exe + full path).
/// Used by the settings "target app" picker, not by clipboard capture.
#[cfg(target_os = "windows")]
pub fn get_foreground_app_info() -> Option<crate::commands::settings::PickedApp> {
    use crate::commands::settings::PickedApp;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() { return None; }

        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if process_id == 0 { return None; }

        let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id).ok()?;

        let mut name_buf = [0u16; MAX_PATH as usize];
        let name_size = GetModuleBaseNameW(process_handle, None, &mut name_buf);
        let exe_name = (name_size > 0).then(|| String::from_utf16_lossy(&name_buf[..name_size as usize]));

        let mut path_buf = [0u16; MAX_PATH as usize];
        let path_size = GetModuleFileNameExW(Some(process_handle), None, &mut path_buf);
        let full_path = (path_size > 0).then(|| String::from_utf16_lossy(&path_buf[..path_size as usize]));

        let app_name = full_path.as_deref()
            .and_then(|p| get_app_description(p))
            .or_else(|| exe_name.clone());

        Some(PickedApp { app_name, exe_name, full_path })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_foreground_app_info() -> Option<crate::commands::settings::PickedApp> {
    None
}

/// Snapshot the current foreground HWND. Called before a helper window steals focus
/// so we can restore the user's target app for keyboard paste routing.
#[cfg(target_os = "windows")]
pub fn capture_prev_foreground() {
    unsafe {
        let hwnd = GetForegroundWindow();
        // Skip our own windows — finding the scratchpad or settings window here means the
        // user was already inside ClipPaste, and restoring it isn't what they want.
        if hwnd.0.is_null() { return; }
        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        let own_pid = std::process::id();
        if process_id != own_pid {
            PREV_FOREGROUND_HWND.store(hwnd.0 as isize, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn capture_prev_foreground() {}

/// Try to restore focus to the previously captured foreground window.
/// Uses the standard Win32 focus-stealing bypass (attach input thread briefly).
/// Returns true if the restore succeeded.
#[cfg(target_os = "windows")]
pub fn restore_prev_foreground() -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, IsWindow, BringWindowToTop,
    };
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};

    let stored = PREV_FOREGROUND_HWND.load(std::sync::atomic::Ordering::SeqCst);
    if stored == 0 { return false; }
    let hwnd = HWND(stored as *mut _);

    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() { return false; }

        // AttachThreadInput trick: Windows blocks SetForegroundWindow from one thread to
        // another unless their input queues are attached. We attach briefly, foreground
        // the target, then detach.
        let mut tid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut tid));
        let our_tid = GetCurrentThreadId();

        let _ = AttachThreadInput(our_tid, tid, true);
        let _ = BringWindowToTop(hwnd);
        let ok = SetForegroundWindow(hwnd).as_bool();
        let _ = AttachThreadInput(our_tid, tid, false);

        ok
    }
}

#[cfg(not(target_os = "windows"))]
pub fn restore_prev_foreground() -> bool { false }

#[cfg(target_os = "windows")]
fn get_clipboard_owner_app_info() -> (Option<String>, Option<String>, Option<String>, Option<String>, bool) {
    unsafe {
        let (hwnd, is_explicit) = match GetClipboardOwner() {
            Ok(h) if !h.0.is_null() => (h, true),
            Err(e) => {
                log::info!("CLIPBOARD: GetClipboardOwner failed: {:?}, falling back to foreground window", e);
                (GetForegroundWindow(), false)
            },
            Ok(_) => {
                log::info!("CLIPBOARD: GetClipboardOwner returned null, falling back to foreground window");
                (GetForegroundWindow(), false)
            }
        };

        if hwnd.0.is_null() {
            return (None, None, None, None, false);
        }

        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        if process_id == 0 {
            return (None, None, None, None, false);
        }

        let process_handle = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
            Ok(h) => h,
            Err(_) => return (None, None, None, None, false),
        };

        let mut name_buffer = [0u16; MAX_PATH as usize];
        let name_size = GetModuleBaseNameW(process_handle, None, &mut name_buffer);
        let exe_name = if name_size > 0 {
            String::from_utf16_lossy(&name_buffer[..name_size as usize])
        } else {
            String::new()
        };

        let mut path_buffer = [0u16; MAX_PATH as usize];
        let path_size = GetModuleFileNameExW(Some(process_handle), None, &mut path_buffer);
        let (app_name, app_icon, full_path) = if path_size > 0 {
            let full_path_str = String::from_utf16_lossy(&path_buffer[..path_size as usize]);

            let desc = get_app_description(&full_path_str);
            let final_name = if let Some(d) = desc {
                Some(d)
            } else if !exe_name.is_empty() {
                Some(exe_name.clone())
            } else {
                None
            };

            let icon = {
                let mut cache = ICON_CACHE.lock();
                if let Some(cached) = cache.get(&full_path_str) {
                    cached.clone()
                } else {
                    let extracted = extract_icon(&full_path_str);
                    cache.put(full_path_str.clone(), extracted.clone());
                    extracted
                }
            };
            (final_name, icon, Some(full_path_str))
        } else {
            (if !exe_name.is_empty() { Some(exe_name.clone()) } else { None }, None, None)
        };

        let exe_val = if !exe_name.is_empty() { Some(exe_name) } else { None };
        (app_name, app_icon, exe_val, full_path, is_explicit)
    }
}

#[cfg(target_os = "macos")]
fn get_clipboard_owner_app_info() -> (Option<String>, Option<String>, Option<String>, Option<String>, bool) {
    use objc2_app_kit::NSWorkspace;

    unsafe {
        let workspace = NSWorkspace::sharedWorkspace();
        let app = match workspace.frontmostApplication() {
            Some(app) => app,
            None => return (None, None, None, None, false),
        };

        let app_name = app.localizedName()
            .map(|s| s.to_string());

        let bundle_id = app.bundleIdentifier()
            .map(|s| s.to_string());

        // Note: Icon extraction skipped due to objc2 version conflicts with window-vibrancy.
        // App icon support for macOS can be added when dependencies are aligned.

        log::info!("CLIPBOARD: macOS source app: {:?}, bundle: {:?}", app_name, bundle_id);
        (app_name, None, bundle_id.clone(), bundle_id, true)
    }
}

#[cfg(target_os = "linux")]
fn get_clipboard_owner_app_info() -> (Option<String>, Option<String>, Option<String>, Option<String>, bool) {
    (None, None, None, None, false)
}

// ========== PLATFORM-SPECIFIC: App description (Windows) ==========

#[cfg(target_os = "windows")]
unsafe fn get_app_description(path: &str) -> Option<String> {
    use std::ffi::c_void;

    let wide_path: Vec<u16> = OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();

    let size = GetFileVersionInfoSizeW(windows::core::PCWSTR(wide_path.as_ptr()), None);
    if size == 0 { return None; }

    let mut data = vec![0u8; size as usize];
    if GetFileVersionInfoW(windows::core::PCWSTR(wide_path.as_ptr()), Some(0), size, data.as_mut_ptr() as *mut _).is_err() {
        return None;
    }

    let mut lang_ptr: *mut c_void = std::ptr::null_mut();
    let mut lang_len: u32 = 0;

    let translation_query = OsStr::new("\\VarFileInfo\\Translation").encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();

    if !VerQueryValueW(data.as_ptr() as *const _, windows::core::PCWSTR(translation_query.as_ptr()), &mut lang_ptr, &mut lang_len).as_bool() {
        return None;
    }

    if lang_len < 4 { return None; }

    let pairs = std::slice::from_raw_parts(lang_ptr as *const u16, (lang_len / 2) as usize);
    let num_pairs = (lang_len / 4) as usize;

    let mut lang_code = pairs[0];
    let mut charset_code = pairs[1];

    for i in 0..num_pairs {
        let code = pairs[i * 2];
        let charset = pairs[i * 2 + 1];

        if code == 0x0804 {
            lang_code = code;
            charset_code = charset;
        }
    }

    let keys = ["FileDescription", "ProductName"];

    for key in keys {
        let query_str = format!("\\StringFileInfo\\{:04x}{:04x}\\{}", lang_code, charset_code, key);
        let query = OsStr::new(&query_str).encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();

        let mut desc_ptr: *mut c_void = std::ptr::null_mut();
        let mut desc_len: u32 = 0;

        if VerQueryValueW(data.as_ptr() as *const _, windows::core::PCWSTR(query.as_ptr()), &mut desc_ptr, &mut desc_len).as_bool() {
             let desc = std::slice::from_raw_parts(desc_ptr as *const u16, desc_len as usize);
             let len = if desc.last() == Some(&0) { desc.len() - 1 } else { desc.len() };
             if len > 0 {
                 return Some(String::from_utf16_lossy(&desc[..len]));
             }
        }
    }

    None
}

// ========== PLATFORM-SPECIFIC: Icon extraction (Windows) ==========

#[cfg(target_os = "windows")]
unsafe fn extract_icon(path: &str) -> Option<String> {
    use image::ImageEncoder;

    let wide_path: Vec<u16> = OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
    let mut shfi = SHFILEINFOW::default();

    SHGetFileInfoW(
        windows::core::PCWSTR(wide_path.as_ptr()),
        windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        Some(&mut shfi as *mut _),
        std::mem::size_of::<SHFILEINFOW>() as u32,
        SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES
    );

    if shfi.hIcon.is_invalid() {
        return None;
    }

    let icon = shfi.hIcon;
    struct IconGuard(windows::Win32::UI::WindowsAndMessaging::HICON);
    impl Drop for IconGuard { fn drop(&mut self) { unsafe { let _ = DestroyIcon(self.0); } } }
    let _guard = IconGuard(icon);

    let mut icon_info = ICONINFO::default();
    if GetIconInfo(icon, &mut icon_info).is_err() { return None; }

    struct BitmapGuard(HBITMAP);
    impl Drop for BitmapGuard { fn drop(&mut self) { unsafe { if !self.0.is_invalid() { let _ = DeleteObject(self.0.into()); } } } }
    let _bm_mask = BitmapGuard(icon_info.hbmMask);
    let _bm_color = BitmapGuard(icon_info.hbmColor);

    let mut bm = BITMAP::default();
    if GetObjectW(icon_info.hbmMask.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _)) == 0 { return None; }

    let width = bm.bmWidth;
    let height = if !icon_info.hbmColor.is_invalid() { bm.bmHeight } else { bm.bmHeight / 2 };

    // Bounds check: reject zero/negative dimensions or unreasonably large icons (>1024px)
    if width <= 0 || height <= 0 || width > 1024 || height > 1024 {
        log::warn!("ICON: Invalid icon dimensions {}x{}, skipping", width, height);
        return None;
    }

    let screen_dc = GetDC(None);
    let mem_dc = CreateCompatibleDC(Some(screen_dc));
    let mem_bm = CreateCompatibleBitmap(screen_dc, width, height);

    let old_obj = SelectObject(mem_dc, mem_bm.into());

    let _ = DrawIconEx(mem_dc, 0, 0, icon, width, height, 0, None, DI_NORMAL);

    let bi = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };

    let mut pixels = vec![0u8; (width * height * 4) as usize];

    GetDIBits(mem_dc, mem_bm, 0, height as u32, Some(pixels.as_mut_ptr() as *mut _), &mut BITMAPINFO { bmiHeader: bi, ..Default::default() }, DIB_RGB_COLORS);

    SelectObject(mem_dc, old_obj);
    let _ = DeleteDC(mem_dc);
    let _ = DeleteObject(mem_bm.into());
    let _ = ReleaseDC(None, screen_dc);

    for chunk in pixels.chunks_exact_mut(4) {
        let b = chunk[0];
        let r = chunk[2];
        chunk[0] = r;
        chunk[2] = b;
    }

    let mut png_data = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
    encoder.write_image(&pixels, width as u32, height as u32, image::ColorType::Rgba8).ok()?;

    Some(BASE64.encode(&png_data))
}

// ========== PLATFORM-SPECIFIC: Paste input simulation ==========

#[cfg(target_os = "windows")]
pub fn send_paste_input() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_MENU, VK_LMENU, VK_RMENU,
        VK_LWIN, VK_RWIN,
    };

    // If the paste was triggered by a shortcut like Ctrl+Enter, the user's physical
    // Ctrl key may still be down when we get here. Injecting a KEYUP isn't enough —
    // many apps check GetAsyncKeyState which reports physical state, so they'd still
    // see Ctrl held and treat Shift+Insert as Ctrl+Shift+Insert. Wait briefly for the
    // user to actually release the modifier.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(400);
    loop {
        let any_held = unsafe {
            ((GetAsyncKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000) != 0
                || ((GetAsyncKeyState(VK_MENU.0 as i32) as u16) & 0x8000) != 0
                || ((GetAsyncKeyState(VK_LWIN.0 as i32) as u16) & 0x8000) != 0
                || ((GetAsyncKeyState(VK_RWIN.0 as i32) as u16) & 0x8000) != 0
        };
        if !any_held || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    unsafe {
        let keyup = |vk| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        };
        let keydown = |vk| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    ..Default::default()
                },
            },
        };

        // Only inject KEYUP for modifiers that are actually held. A stray KEYUP for
        // Alt or Win without a preceding KEYDOWN is treated by apps as a press-and-
        // release tap: Alt activates the menu bar (Notepad File/Edit/…), and Win
        // opens the Start menu. This also matters after Ctrl+Win+Arrow virtual-
        // desktop switches, where Win may still briefly register as held.
        let ctrl_held = ((GetAsyncKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000) != 0;
        let menu_held = ((GetAsyncKeyState(VK_MENU.0 as i32) as u16) & 0x8000) != 0;
        let lwin_held = ((GetAsyncKeyState(VK_LWIN.0 as i32) as u16) & 0x8000) != 0;
        let rwin_held = ((GetAsyncKeyState(VK_RWIN.0 as i32) as u16) & 0x8000) != 0;

        let mut release_mods: Vec<INPUT> = Vec::new();
        if ctrl_held {
            release_mods.push(keyup(VK_LCONTROL));
            release_mods.push(keyup(VK_RCONTROL));
            release_mods.push(keyup(VK_CONTROL));
        }
        if menu_held {
            release_mods.push(keyup(VK_LMENU));
            release_mods.push(keyup(VK_RMENU));
            release_mods.push(keyup(VK_MENU));
        }
        if lwin_held {
            release_mods.push(keyup(VK_LWIN));
        }
        if rwin_held {
            release_mods.push(keyup(VK_RWIN));
        }
        if !release_mods.is_empty() {
            SendInput(&release_mods, std::mem::size_of::<INPUT>() as i32);
        }

        let inputs = vec![
            keydown(VK_SHIFT),
            keydown(VK_INSERT),
            keyup(VK_INSERT),
            keyup(VK_SHIFT),
        ];
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "macos")]
pub fn send_paste_input() {
    use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    log::info!("CLIPBOARD: macOS send_paste_input: sending Cmd+V");

    let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s,
        Err(_) => {
            log::error!("CLIPBOARD: Failed to create CGEventSource");
            return;
        }
    };

    let v_keycode: CGKeyCode = 9; // 'v' key on macOS

    // Key down: Cmd+V
    if let Ok(key_down) = CGEvent::new_keyboard_event(source.clone(), v_keycode, true) {
        key_down.set_flags(CGEventFlags::CGEventFlagCommand);
        key_down.post(core_graphics::event::CGEventTapLocation::HID);
    }

    // Key up: Cmd+V
    if let Ok(key_up) = CGEvent::new_keyboard_event(source, v_keycode, false) {
        key_up.set_flags(CGEventFlags::CGEventFlagCommand);
        key_up.post(core_graphics::event::CGEventTapLocation::HID);
    }
}

#[cfg(target_os = "linux")]
pub fn send_paste_input() {
    log::warn!("CLIPBOARD: send_paste_input not implemented on Linux");
}
