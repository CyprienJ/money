use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::importing::csv::{self, MappingSuggestion, NewCsvMapping, SavedCsvMapping};

#[tauri::command]
#[specta::specta]
pub async fn list_csv_mappings(pool: State<'_, SqlitePool>) -> AppResult<Vec<SavedCsvMapping>> {
    csv::list(&pool).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_csv_mapping(pool: State<'_, SqlitePool>, id: i64) -> AppResult<SavedCsvMapping> {
    csv::get(&pool, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_csv_mapping(
    pool: State<'_, SqlitePool>,
    mapping: NewCsvMapping,
) -> AppResult<SavedCsvMapping> {
    csv::create(&pool, mapping).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_csv_mapping(
    pool: State<'_, SqlitePool>,
    id: i64,
    changes: NewCsvMapping,
) -> AppResult<SavedCsvMapping> {
    csv::update(&pool, id, changes).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_csv_mapping(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    csv::delete(&pool, id).await
}

/// Saved mappings that read the CSV file at `path`, the most lines read first.
#[tauri::command]
#[specta::specta]
pub async fn suggest_csv_mappings(
    pool: State<'_, SqlitePool>,
    path: String,
) -> AppResult<Vec<MappingSuggestion>> {
    let bytes = std::fs::read(path)?;
    csv::suggest(&pool, &bytes).await
}
