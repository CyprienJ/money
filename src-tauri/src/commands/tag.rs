use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::tag::{self, Tag};

/// Every tag, by name, with the count of its operations.
#[tauri::command]
#[specta::specta]
pub async fn list_tags(pool: State<'_, SqlitePool>) -> AppResult<Vec<Tag>> {
    tag::list(&pool).await
}

/// Renames a tag; refused when another tag already has this name.
#[tauri::command]
#[specta::specta]
pub async fn rename_tag(pool: State<'_, SqlitePool>, id: i64, name: String) -> AppResult<Tag> {
    tag::rename(&pool, id, &name).await
}

/// Deletes a tag and removes it from its operations.
#[tauri::command]
#[specta::specta]
pub async fn delete_tag(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    tag::delete(&pool, id).await
}
