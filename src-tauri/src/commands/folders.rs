use tauri::Emitter;
use std::sync::Arc;
use crate::database::Database;
use crate::models::{Folder, FolderItem};

#[tauri::command]
pub async fn get_folders(db: tauri::State<'_, Arc<Database>>) -> Result<Vec<FolderItem>, String> {
    let pool = &db.pool;

    let folders: Vec<Folder> = sqlx::query_as(r#"SELECT * FROM folders ORDER BY position, id"#)
        .fetch_all(pool).await.map_err(|e| e.to_string())?;

    // Get counts for all folders in one query
    let counts: Vec<(i64, i64)> = sqlx::query_as(r#"
        SELECT folder_id, COUNT(*) as count
        FROM clips
        WHERE folder_id IS NOT NULL
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
