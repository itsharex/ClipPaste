use crate::database::Database;
use crate::models::{Scratchpad, ScratchpadItem};
use std::sync::Arc;
use tauri::AppHandle;

fn scratchpad_to_item(s: Scratchpad) -> ScratchpadItem {
    ScratchpadItem {
        id: s.id.to_string(),
        uuid: s.uuid.clone(),
        title: s.title,
        content: s.content,
        fields_json: s.fields_json,
        is_pinned: s.is_pinned,
        color: s.color,
        position: s.position,
        created_at: s.created_at.to_rfc3339(),
        updated_at: s.updated_at.map(|dt| dt.to_rfc3339()),
    }
}

#[tauri::command]
pub async fn get_scratchpads(
    db: tauri::State<'_, Arc<Database>>,
) -> Result<Vec<ScratchpadItem>, String> {
    let rows: Vec<Scratchpad> = sqlx::query_as(
        "SELECT id, uuid, title, content, fields_json, is_pinned, color, position, created_at, updated_at
         FROM scratchpads ORDER BY is_pinned DESC, position ASC, id ASC"
    )
    .fetch_all(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(scratchpad_to_item).collect())
}

#[tauri::command]
pub async fn create_scratchpad(
    title: String,
    content: String,
    fields_json: Option<String>,
    db: tauri::State<'_, Arc<Database>>,
) -> Result<ScratchpadItem, String> {
    let uuid = uuid::Uuid::new_v4().to_string();

    let max_pos: Option<i64> = sqlx::query_scalar("SELECT MAX(position) FROM scratchpads")
        .fetch_one(&db.pool)
        .await
        .map_err(|e| e.to_string())?;
    let position = max_pos.unwrap_or(0) + 1;

    sqlx::query(
        "INSERT INTO scratchpads (uuid, title, content, fields_json, position, updated_at) VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"
    )
    .bind(&uuid)
    .bind(&title)
    .bind(&content)
    .bind(&fields_json)
    .bind(position)
    .execute(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    let row: Scratchpad = sqlx::query_as(
        "SELECT id, uuid, title, content, fields_json, is_pinned, color, position, created_at, updated_at FROM scratchpads WHERE uuid = ?"
    )
    .bind(&uuid)
    .fetch_one(&db.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(scratchpad_to_item(row))
}

#[tauri::command]
pub async fn update_scratchpad(
    id: String,
    title: Option<String>,
    content: Option<String>,
    color: Option<String>,
    fields_json: Option<String>,
    db: tauri::State<'_, Arc<Database>>,
) -> Result<(), String> {
    let id_num: i64 = id.parse().map_err(|_| "Invalid scratchpad id")?;

    if let Some(t) = &title {
        sqlx::query("UPDATE scratchpads SET title = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(t).bind(id_num)
            .execute(&db.pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(c) = &content {
        sqlx::query("UPDATE scratchpads SET content = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(c).bind(id_num)
            .execute(&db.pool).await.map_err(|e| e.to_string())?;
    }
    if let Some(col) = &color {
        let val = if col.is_empty() { None } else { Some(col.as_str()) };
        sqlx::query("UPDATE scratchpads SET color = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(val).bind(id_num)
            .execute(&db.pool).await.map_err(|e| e.to_string())?;
    }
    // fields_json: pass "" to clear, or JSON string to set
    if let Some(f) = &fields_json {
        let val = if f.is_empty() { None } else { Some(f.as_str()) };
        sqlx::query("UPDATE scratchpads SET fields_json = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(val).bind(id_num)
            .execute(&db.pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_scratchpad(
    id: String,
    db: tauri::State<'_, Arc<Database>>,
) -> Result<(), String> {
    let id_num: i64 = id.parse().map_err(|_| "Invalid scratchpad id")?;

    let uuid: Option<String> = sqlx::query_scalar("SELECT uuid FROM scratchpads WHERE id = ?")
        .bind(id_num)
        .fetch_optional(&db.pool)
        .await
        .map_err(|e| e.to_string())?;

    if let Some(uuid) = uuid {
        crate::sync::record_tombstone(&db, &uuid, "scratchpad").await.ok();
    }

    sqlx::query("DELETE FROM scratchpads WHERE id = ?")
        .bind(id_num)
        .execute(&db.pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn reorder_scratchpads(
    ids: Vec<String>,
    db: tauri::State<'_, Arc<Database>>,
) -> Result<(), String> {
    for (i, id) in ids.iter().enumerate() {
        let id_num: i64 = id.parse().map_err(|_| "Invalid scratchpad id")?;
        sqlx::query("UPDATE scratchpads SET position = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(i as i64)
            .bind(id_num)
            .execute(&db.pool)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn toggle_scratchpad_pin(
    id: String,
    db: tauri::State<'_, Arc<Database>>,
) -> Result<bool, String> {
    let id_num: i64 = id.parse().map_err(|_| "Invalid scratchpad id")?;
    let current: bool = sqlx::query_scalar("SELECT is_pinned FROM scratchpads WHERE id = ?")
        .bind(id_num).fetch_one(&db.pool).await.map_err(|e| e.to_string())?;
    let new_val = !current;
    sqlx::query("UPDATE scratchpads SET is_pinned = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(new_val).bind(id_num).execute(&db.pool).await.map_err(|e| e.to_string())?;
    Ok(new_val)
}

#[tauri::command]
pub async fn scratchpad_paste(
    text: String,
    app: AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    use sha2::{Sha256, Digest};

    // Write text to clipboard
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    super::helpers::clipboard_write_text(&app, &text, &hash).await?;

    // Hide scratchpad window (no animation, just hide)
    let _ = window.hide();

    // Auto-paste if enabled
    let auto_paste = crate::clipboard::get_cached_setting("auto_paste")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    if auto_paste {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            #[cfg(target_os = "windows")]
            crate::clipboard::send_paste_input();
            let _ = &window;
        });
    }

    Ok(())
}
