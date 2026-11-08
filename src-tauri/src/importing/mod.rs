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
pub mod source_document;
pub mod statement_import;
pub mod trade_import;

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
    /// Parts of a split line of a QIF file, consistent with its amount: at least two, in
    /// several categories, non-zero and of its sign, summing to it. Empty otherwise.
    pub splits: Vec<RawSplit>,
    /// Page of the line in a PDF statement, from 1.
    pub source_page: Option<u32>,
    /// Line in a text file (CSV, OFX, QIF), from 1.
    pub source_line: Option<u32>,
}

/// A part of a split line of a QIF file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RawSplit {
    /// Category given by the file, as `RawLine::source_category`; `None` when empty.
    pub source_category: Option<String>,
    pub amount_cents: i64,
}

#[cfg(test)]
impl RawLine {
    /// The line without its position in the file, to compare with an expected line.
    pub fn unplaced(self) -> RawLine {
        RawLine {
            source_page: None,
            source_line: None,
            ..self
        }
    }
}

/// Lines without their position in the file, for tests.
#[cfg(test)]
pub fn unplaced(lines: Vec<RawLine>) -> Vec<RawLine> {
    lines.into_iter().map(RawLine::unplaced).collect()
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
