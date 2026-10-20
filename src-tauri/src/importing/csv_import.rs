//! CSV import workflow: preview of a file, then import of the lines the user kept.

use std::collections::HashSet;
use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::RawLine;
use super::batch::{
    self, BalanceCheckStatus, ImportBatch, ImportError, ImportLine, NewImportBatch, SourceType,
};
use super::csv;
use super::duplicates::{self, DuplicateStatus, ExistingOperation};
use crate::account;
use crate::error::AppResult;
use crate::labeling::{memory, normalize_label};

/// A statement line in the import preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLine {
    pub line: RawLine,
    pub normalized_label: String,
    pub duplicate: DuplicateStatus,
    /// Dated before the account opening: already counted in the opening balance, never
    /// imported.
    pub before_opening: bool,
    /// Checked in the preview unless the user unchecks it: new lines only.
    pub included_by_default: bool,
    /// Category remembered for this label, given at import; `None`: unknown label.
    pub category_id: Option<i64>,
}

/// A CSV file to import into an account with a saved mapping.
#[derive(Debug, Clone, Copy)]
pub struct CsvFile<'a> {
    pub account_id: i64,
    pub mapping_id: i64,
    pub file_name: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub file_name: String,
    /// To check at import time that the file did not change.
    pub file_hash: String,
    /// Statement lines in file order; the import refers to them by index.
    pub lines: Vec<PreviewLine>,
    /// Existing operations that lines duplicate or may duplicate.
    pub matched_operations: Vec<ExistingOperation>,
    /// Earlier imports of the same file, in any account.
    pub previous_imports: Vec<ImportBatch>,
    pub first_date: Option<NaiveDate>,
    pub last_date: Option<NaiveDate>,
}

/// Reads `bytes` with the saved mapping and compares its lines with the account.
pub async fn preview(pool: &SqlitePool, file: CsvFile<'_>) -> AppResult<ImportPreview> {
    let account = account::get(pool, file.account_id).await?;
    let mapping = csv::get(pool, file.mapping_id).await?;
    let raw_lines = csv::parse(file.bytes, &mapping.mapping)?;

    let existing = duplicates::existing_operations(pool, file.account_id, &raw_lines).await?;
    let statuses = duplicates::classify(&raw_lines, &existing);
    let referenced: HashSet<i64> = statuses
        .iter()
        .flat_map(|status| match status {
            DuplicateStatus::New => Vec::new(),
            DuplicateStatus::Duplicate { operation_id } => vec![*operation_id],
            DuplicateStatus::Ambiguous { candidate_ids, .. } => candidate_ids.clone(),
        })
        .collect();

    let file_hash = batch::file_hash(file.bytes);
    let previous_imports = batch::find_by_hash(pool, &file_hash).await?;
    let normalized: Vec<String> = raw_lines
        .iter()
        .map(|l| normalize_label(&l.label))
        .collect();
    let remembered = {
        let labels: Vec<&str> = normalized.iter().map(String::as_str).collect();
        let mut conn = pool.acquire().await?;
        memory::recall(&mut conn, &labels).await?
    };
    let lines = raw_lines
        .into_iter()
        .zip(statuses)
        .zip(normalized)
        .map(|((line, duplicate), normalized_label)| {
            let before_opening = line.date < account.opening_date;
            PreviewLine {
                category_id: remembered.get(&normalized_label).copied(),
                normalized_label,
                before_opening,
                included_by_default: !before_opening && duplicate.included_by_default(),
                duplicate,
                line,
            }
        })
        .collect::<Vec<_>>();

    Ok(ImportPreview {
        file_name: file.file_name.to_owned(),
        file_hash,
        first_date: lines.iter().map(|preview| preview.line.date).min(),
        last_date: lines.iter().map(|preview| preview.line.date).max(),
        lines,
        matched_operations: existing
            .into_iter()
            .filter(|operation| referenced.contains(&operation.id))
            .collect(),
        previous_imports,
    })
}

/// A preview line to import, with the category the user kept or chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSelection {
    /// Index of the line in the preview.
    pub index: u32,
    /// `None`: to categorize.
    pub category_id: Option<i64>,
}

/// Imports the selected lines as a new batch, after a backup into `backup_dir`. The file is
/// read again and must not have changed since the preview; exact duplicates and lines dated
/// before the account opening are refused. A category that differs from the proposed one is
/// a choice of the user, remembered for the label.
pub async fn import(
    pool: &SqlitePool,
    backup_dir: &Path,
    file: CsvFile<'_>,
    expected_hash: &str,
    selections: &[ImportSelection],
) -> AppResult<ImportBatch> {
    let file_hash = batch::file_hash(file.bytes);
    if file_hash != expected_hash {
        return Err(ImportError::FileChanged.into());
    }
    let preview = preview(pool, file).await?;

    let mut seen = HashSet::new();
    let mut lines = Vec::new();
    for &ImportSelection { index, category_id } in selections {
        if !seen.insert(index) {
            continue;
        }
        let line = preview
            .lines
            .get(index as usize)
            .ok_or(ImportError::LineOutOfRange(index))?;
        if matches!(line.duplicate, DuplicateStatus::Duplicate { .. }) {
            return Err(ImportError::DuplicateLine(index).into());
        }
        lines.push(ImportLine {
            line: line.line.clone(),
            category_id,
            chosen: category_id.is_some() && category_id != line.category_id,
        });
    }

    batch::import(
        pool,
        backup_dir,
        NewImportBatch {
            account_id: file.account_id,
            source_type: SourceType::Csv,
            source_file_name: file.file_name.to_owned(),
            source_file_hash: file_hash,
            statement_start_balance_cents: None,
            statement_end_balance_cents: None,
            balance_check_status: BalanceCheckStatus::NotAvailable,
        },
        &lines,
    )
    .await
}

#[cfg(test)]
mod tests;
