//! Import batches, duplicate detection, balance check and backup.

pub mod balance;
pub mod batch;
pub mod csv;
pub mod duplicates;
pub mod interchange;
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "statements are parsed by the bank parsers of V1-05.2"
    )
)]
pub mod pdf;
pub mod source_category;
pub mod statement_import;

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;

/// A statement line as read from the bank file, before normalization and duplicate detection.
/// Every parser (CSV mapping, bank-specific PDF parser) produces this structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RawLine {
    pub date: NaiveDate,
    pub value_date: Option<NaiveDate>,
    /// Label exactly as written by the bank, only trimmed.
    pub label: String,
    /// Negative for a debit, positive for a credit.
    pub amount_cents: i64,
    /// Memo of an OFX or QIF line, kept as the note of the operation.
    pub note: Option<String>,
    /// Category given by a QIF file, e.g. "Alimentation:Courses"; `[Compte]` for a transfer.
    pub source_category: Option<String>,
}

/// What a parser reads from a statement file (PDF, OFX, QIF).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub lines: Vec<RawLine>,
    /// Balance before the first line, when the statement gives it.
    pub start_balance_cents: Option<i64>,
    /// Balance after the last line, when the statement gives it.
    pub end_balance_cents: Option<i64>,
    /// First and last days covered by the statement, when it gives them.
    pub period: Option<(NaiveDate, NaiveDate)>,
}
