use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::importing::batch::{self, ImportBatch};
use crate::importing::csv::{self, CsvEncoding};
use crate::importing::pdf::{self, SupportedBank};
use crate::importing::statement_import::{
    self, ImportFormat, ImportPreview, ImportSelection, StatementFile,
};

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

/// Reads the statement file at `path` (CSV with a saved mapping, or PDF of a supported
/// bank) and compares it with the account.
#[tauri::command]
#[specta::specta]
pub async fn preview_import(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    path: String,
    account_id: i64,
    format: ImportFormat,
) -> AppResult<ImportPreview> {
    let bytes = std::fs::read(&path)?;
    let pdfium_dir = pdfium_dir(&app)?;
    let file = statement_file(&path, account_id, format, &bytes, &pdfium_dir);
    statement_import::preview(&pool, file).await
}

/// Imports the selected lines of the statement file at `path`, with their categories.
/// `file_hash` comes from the preview and guards against a file changed in between.
#[tauri::command]
#[specta::specta]
pub async fn import_statement(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    path: String,
    account_id: i64,
    format: ImportFormat,
    file_hash: String,
    selections: Vec<ImportSelection>,
) -> AppResult<ImportBatch> {
    let bytes = std::fs::read(&path)?;
    let pdfium_dir = pdfium_dir(&app)?;
    let file = statement_file(&path, account_id, format, &bytes, &pdfium_dir);
    statement_import::import(&pool, &backup_dir(&app)?, file, &file_hash, &selections).await
}

fn statement_file<'a>(
    path: &'a str,
    account_id: i64,
    format: ImportFormat,
    bytes: &'a [u8],
    pdfium_dir: &'a Path,
) -> StatementFile<'a> {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    StatementFile {
        account_id,
        file_name,
        bytes,
        format,
        pdfium_dir,
    }
}

/// Banks whose PDF statements can be imported.
#[tauri::command]
#[specta::specta]
pub fn supported_banks() -> Vec<SupportedBank> {
    pdf::supported_banks()
}

/// Bank of the PDF statement at `path`, `null` when no parser recognizes it.
#[tauri::command]
#[specta::specta]
pub async fn detect_pdf_statement(
    app: AppHandle,
    path: String,
) -> AppResult<Option<SupportedBank>> {
    let bytes = std::fs::read(path)?;
    let document = pdf::extract(&pdfium_dir(&app)?, &bytes)?;
    Ok(pdf::detect(&document).map(SupportedBank::of))
}

/// Directory of the PDFium library, bundled as a resource (`tauri.<os>.conf.json`).
fn pdfium_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let resources = app
        .path()
        .resource_dir()
        .map_err(|error| AppError::Io(std::io::Error::other(error)))?;
    Ok(resources.join("pdfium"))
}

/// `backups/` in the application data directory (README, storage).
fn backup_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::Io(std::io::Error::other(error)))?;
    Ok(data_dir.join("backups"))
}
