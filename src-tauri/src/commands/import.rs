use std::path::{Path, PathBuf};

use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::importing::batch::{self, ImportBatch};
use crate::importing::csv::{self, CsvEncoding};
use crate::importing::pdf::{self, SupportedBank};
use crate::importing::source_category::SourceCategoryChoice;
use crate::importing::statement_import::{
    self, ImportFormat, ImportPreview, ImportSelection, StatementFile,
};
use crate::labeling::classifier::ClassifierCache;
use crate::operation::schedule::matching;
use crate::operation::transfer;

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

/// Deletes an import and all its operations, with the files of their attachments.
#[tauri::command]
#[specta::specta]
pub async fn cancel_import(app: AppHandle, pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    batch::cancel(&pool, id).await?;
    crate::attachment::sweep(&pool, &super::app::attachments_dir(&app)?).await?;
    Ok(())
}

/// First lines of the file at `path`, decoded with `encoding`.
#[tauri::command]
#[specta::specta]
pub async fn read_file_head(path: String, encoding: CsvEncoding) -> AppResult<Vec<String>> {
    let bytes = std::fs::read(path)?;
    Ok(csv::head(&bytes, encoding, HEAD_LINES))
}

/// Reads the statement file at `path` (CSV with a saved mapping, or PDF of a supported
/// bank) and compares it with the account. Lines of unknown labels get a suggested category.
#[tauri::command]
#[specta::specta]
pub async fn preview_import(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    classifier: State<'_, ClassifierCache>,
    path: String,
    account_id: i64,
    format: ImportFormat,
) -> AppResult<ImportPreview> {
    let bytes = std::fs::read(&path)?;
    let pdfium_dir = pdfium_dir(&app)?;
    let file = statement_file(&path, account_id, format, &bytes, &pdfium_dir);
    let mut preview = statement_import::preview(&pool, file).await?;
    if let Some(classifier) = classifier.get(&pool).await? {
        statement_import::add_suggestions(&mut preview, &classifier);
    }
    Ok(preview)
}

/// Result of an import.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub batch: ImportBatch,
    /// Internal transfers linked automatically with operations of other accounts.
    pub linked_transfers: u32,
    /// Occurrences of scheduled operations realized automatically by the new operations.
    pub matched_occurrences: u32,
}

/// Imports the selected lines of the statement file at `path`, with their categories, then
/// links the certain internal transfers and schedule occurrences of the new operations.
/// `file_hash` comes from the preview and guards against a file changed in between;
/// `source_categories` are the categories chosen for the categories of a QIF file.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments, reason = "the arguments of a command")]
pub async fn import_statement(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    path: String,
    account_id: i64,
    format: ImportFormat,
    file_hash: String,
    selections: Vec<ImportSelection>,
    source_categories: Vec<SourceCategoryChoice>,
) -> AppResult<ImportOutcome> {
    let bytes = std::fs::read(&path)?;
    let pdfium_dir = pdfium_dir(&app)?;
    let file = statement_file(&path, account_id, format, &bytes, &pdfium_dir);
    let batch = statement_import::import(
        &pool,
        &super::app::backups_dir(&app)?,
        file,
        &file_hash,
        &selections,
        &source_categories,
    )
    .await?;
    let linked_transfers = transfer::link_certain(&pool, Some(batch.id)).await?;
    let matched_occurrences =
        matching::match_certain(&pool, super::operation::today(), Some(batch.id)).await?;
    Ok(ImportOutcome {
        batch,
        linked_transfers,
        matched_occurrences,
    })
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
