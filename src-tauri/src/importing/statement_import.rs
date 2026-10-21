//! Statement import workflow, for CSV and PDF files: preview of a file, then import of the
//! lines the user kept.

use std::collections::HashSet;
use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::RawLine;
use super::balance::{self, BalanceCheck};
use super::batch::{self, ImportBatch, ImportError, ImportLine, NewImportBatch, SourceType};
use super::csv;
use super::duplicates::{self, DuplicateStatus, ExistingOperation};
use super::pdf::{self, PdfError};
use crate::account;
use crate::error::AppResult;
use crate::labeling::rules::Rules;
use crate::labeling::{memory, normalize_label};
use crate::payee;

/// A statement line in the import preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLine {
    pub line: RawLine,
    pub normalized_label: String,
    /// Payee of the label, or the name its new payee will get.
    pub payee_name: String,
    pub duplicate: DuplicateStatus,
    /// Dated before the account opening: already counted in the opening balance, never
    /// imported.
    pub before_opening: bool,
    /// Checked in the preview unless the user unchecks it: new lines only.
    pub included_by_default: bool,
    /// Category given at import: by the first matching rule, or else remembered for this
    /// label; `None`: unknown label.
    pub category_id: Option<i64>,
    /// Text of the rule that gave the category; `None` when remembered or unknown.
    pub rule_pattern: Option<String>,
}

/// How to read a statement file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ImportFormat {
    /// CSV export read with a saved mapping.
    Csv { mapping_id: i64 },
    /// PDF statement read by the parser of its bank, detected from its content.
    Pdf,
}

/// A statement file to import into an account.
#[derive(Debug, Clone, Copy)]
pub struct StatementFile<'a> {
    pub account_id: i64,
    pub file_name: &'a str,
    pub bytes: &'a [u8],
    pub format: ImportFormat,
    /// Directory of the PDFium library, for PDF files.
    pub pdfium_dir: &'a Path,
}

/// Lines and balances read from a statement file.
struct ReadStatement {
    lines: Vec<RawLine>,
    /// Bank recognized from a PDF statement.
    bank: Option<String>,
    start_balance_cents: Option<i64>,
    end_balance_cents: Option<i64>,
    period: Option<(NaiveDate, NaiveDate)>,
}

async fn read(pool: &SqlitePool, file: StatementFile<'_>) -> AppResult<ReadStatement> {
    match file.format {
        ImportFormat::Csv { mapping_id } => {
            let mapping = csv::get(pool, mapping_id).await?;
            Ok(ReadStatement {
                lines: csv::parse(file.bytes, &mapping.mapping)?,
                bank: None,
                start_balance_cents: None,
                end_balance_cents: None,
                period: None,
            })
        }
        ImportFormat::Pdf => {
            let document = pdf::extract(file.pdfium_dir, file.bytes)?;
            let parser = pdf::detect(&document).ok_or(PdfError::UnknownFormat)?;
            let statement = parser.parse(&document)?;
            Ok(ReadStatement {
                lines: statement.lines,
                bank: Some(parser.bank().to_owned()),
                start_balance_cents: statement.start_balance_cents,
                end_balance_cents: statement.end_balance_cents,
                period: statement.period,
            })
        }
    }
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
    /// Statement period, or else the dates of its first and last lines.
    pub first_date: Option<NaiveDate>,
    pub last_date: Option<NaiveDate>,
    /// Bank recognized from a PDF statement ("Format reconnu : …").
    pub bank: Option<String>,
    /// Balances printed on the statement, when it gives them.
    pub start_balance_cents: Option<i64>,
    pub end_balance_cents: Option<i64>,
    /// `start balance + Σ lines = end balance`, over every line read.
    pub balance_check: BalanceCheck,
}

/// Reads the statement file and compares its lines with the account.
pub async fn preview(pool: &SqlitePool, file: StatementFile<'_>) -> AppResult<ImportPreview> {
    let account = account::get(pool, file.account_id).await?;
    let statement = read(pool, file).await?;
    let balance_check = balance::check(
        statement.start_balance_cents,
        &statement.lines,
        statement.end_balance_cents,
    );
    let raw_lines = statement.lines;

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
    let (rules, remembered, payee_names) = {
        let labels: Vec<&str> = normalized.iter().map(String::as_str).collect();
        let mut conn = pool.acquire().await?;
        (
            Rules::load(&mut conn).await?,
            memory::recall(&mut conn, &labels).await?,
            payee::names_of(&mut conn, &labels).await?,
        )
    };
    let lines = raw_lines
        .into_iter()
        .zip(statuses)
        .zip(normalized)
        .map(|((line, duplicate), normalized_label)| {
            let before_opening = line.date < account.opening_date;
            let rule = rules.find(&normalized_label);
            PreviewLine {
                category_id: rule
                    .map(|(category_id, _)| category_id)
                    .or_else(|| remembered.get(&normalized_label).copied()),
                rule_pattern: rule.map(|(_, pattern)| pattern.to_owned()),
                payee_name: payee_names
                    .get(&normalized_label)
                    .cloned()
                    .unwrap_or_default(),
                normalized_label,
                before_opening,
                included_by_default: !before_opening && duplicate.included_by_default(),
                duplicate,
                line,
            }
        })
        .collect::<Vec<_>>();

    let (first_date, last_date) = match statement.period {
        Some((first, last)) => (Some(first), Some(last)),
        None => (
            lines.iter().map(|preview| preview.line.date).min(),
            lines.iter().map(|preview| preview.line.date).max(),
        ),
    };
    Ok(ImportPreview {
        file_name: file.file_name.to_owned(),
        file_hash,
        first_date,
        last_date,
        bank: statement.bank,
        start_balance_cents: statement.start_balance_cents,
        end_balance_cents: statement.end_balance_cents,
        balance_check,
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
    file: StatementFile<'_>,
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
            source_type: match file.format {
                ImportFormat::Csv { .. } => SourceType::Csv,
                ImportFormat::Pdf => SourceType::Pdf,
            },
            source_file_name: file.file_name.to_owned(),
            source_file_hash: file_hash,
            statement_start_balance_cents: preview.start_balance_cents,
            statement_end_balance_cents: preview.end_balance_cents,
            balance_check_status: preview.balance_check.status,
        },
        &lines,
    )
    .await
}

#[cfg(test)]
mod tests;
