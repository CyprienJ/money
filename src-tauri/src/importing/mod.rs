//! Import batches, duplicate detection, balance check and backup.

pub mod csv;
pub mod pdf;

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
