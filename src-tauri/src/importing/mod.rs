//! Import batches, duplicate detection, balance check and backup.

pub mod balance;
pub mod batch;
pub mod csv;
pub mod duplicates;
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "statements are parsed by the bank parsers of V1-05.2"
    )
)]
pub mod pdf;
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
}
