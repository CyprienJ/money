use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::importing::batch::{self, ImportBatch};
use crate::importing::csv::{self, CsvEncoding};
use crate::importing::csv_import::{self, CsvFile, ImportPreview, ImportSelection};

/// Lines shown when defining a CSV mapping.
const HEAD_LINES: usize = 12;

/// Imports of an account, most recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_import_batches(
    pool: State<'_, SqlitePool>,
    account_id: i64,
) -> AppResult<Vec<ImportBatch>> {
    batch::list(&pool, account_id).await
}

/// Deletes an import and all its operations.
#[tauri::command]
#[specta::specta]
pub async fn cancel_import(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    batch::cancel(&pool, id).await
}

/// First lines of the file at `path`, decoded with `encoding`.
#[tauri::command]
#[specta::specta]
pub async fn read_file_head(path: String, encoding: CsvEncoding) -> AppResult<Vec<String>> {
    let bytes = std::fs::read(path)?;
    Ok(csv::head(&bytes, encoding, HEAD_LINES))
}

/// Reads the CSV file at `path` with a saved mapping and compares it with the account.
#[tauri::command]
#[specta::specta]
pub async fn preview_csv_import(
    pool: State<'_, SqlitePool>,
    path: String,
    account_id: i64,
    mapping_id: i64,
) -> AppResult<ImportPreview> {
    let bytes = std::fs::read(&path)?;
    let file = csv_file(&path, account_id, mapping_id, &bytes);
    csv_import::preview(&pool, file).await
}

/// Imports the selected lines of the CSV file at `path`, with their categories.
/// `file_hash` comes from the preview and guards against a file changed in between.
#[tauri::command]
#[specta::specta]
pub async fn import_csv(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    path: String,
    account_id: i64,
    mapping_id: i64,
    file_hash: String,
    selections: Vec<ImportSelection>,
) -> AppResult<ImportBatch> {
    let bytes = std::fs::read(&path)?;
    let file = csv_file(&path, account_id, mapping_id, &bytes);
    csv_import::import(&pool, &backup_dir(&app)?, file, &file_hash, &selections).await
}

fn csv_file<'a>(path: &'a str, account_id: i64, mapping_id: i64, bytes: &'a [u8]) -> CsvFile<'a> {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    CsvFile {
        account_id,
        mapping_id,
        file_name,
        bytes,
    }
}

/// `backups/` in the application data directory (README, storage).
fn backup_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::Io(std::io::Error::other(error)))?;
    Ok(data_dir.join("backups"))
}
