use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::app::statements_dir;
use crate::error::{AppError, AppResult};
use crate::importing::source_document::{self, SourceDocument};

/// The kept file of an import: name, date and how it is shown.
#[tauri::command]
#[specta::specta]
pub async fn source_document(
    pool: State<'_, SqlitePool>,
    import_batch_id: i64,
) -> AppResult<SourceDocument> {
    source_document::get(&pool, import_batch_id).await
}

/// Text of a kept CSV, OFX or QIF file.
#[tauri::command]
#[specta::specta]
pub async fn source_document_text(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    import_batch_id: i64,
) -> AppResult<String> {
    source_document::text_of(&pool, &statements_dir(&app)?, import_batch_id).await
}

/// Opens the kept file of an import with the default application of the system.
#[tauri::command]
#[specta::specta]
pub async fn open_source_document(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    import_batch_id: i64,
) -> AppResult<()> {
    let path = source_document::path_of(&pool, &statements_dir(&app)?, import_batch_id).await?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|error| AppError::Io(std::io::Error::other(error)))
}
