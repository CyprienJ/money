use std::path::Path;

use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::app::attachments_dir;
use crate::attachment::{self, Attachment};
use crate::error::{AppError, AppResult};

/// Attachments of an operation, oldest first.
#[tauri::command]
#[specta::specta]
pub async fn list_attachments(
    pool: State<'_, SqlitePool>,
    operation_id: i64,
) -> AppResult<Vec<Attachment>> {
    attachment::list(&pool, operation_id).await
}

/// Copies the file at `path` next to the data file and attaches it to an operation.
#[tauri::command]
#[specta::specta]
pub async fn add_attachment(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    operation_id: i64,
    path: String,
) -> AppResult<Attachment> {
    attachment::add(
        &pool,
        &attachments_dir(&app)?,
        operation_id,
        Path::new(&path),
    )
    .await
}

/// Deletes an attachment and its file.
#[tauri::command]
#[specta::specta]
pub async fn delete_attachment(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    id: i64,
) -> AppResult<()> {
    attachment::delete(&pool, &attachments_dir(&app)?, id).await
}

/// Opens an attachment with the default application of the system.
#[tauri::command]
#[specta::specta]
pub async fn open_attachment(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    id: i64,
) -> AppResult<()> {
    let path = attachment::path_of(&pool, &attachments_dir(&app)?, id).await?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|error| AppError::Io(std::io::Error::other(error)))
}
