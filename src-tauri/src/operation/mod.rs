//! Operations, splits and categories.

pub mod category;
mod repository;
pub mod same_label;
pub mod schedule;
pub mod search;
#[cfg(test)]
mod tests;
pub mod transfer;

pub(crate) use repository::{check_account, internal_transfer_flag, timestamp};
pub use repository::{create, delete, get, set_category, update};
#[cfg(test)]
pub use search::list;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::ErrorKind;

/// An operation with its category, or its parts when it is split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: i64,
    pub account_id: i64,
    pub date: NaiveDate,
    pub value_date: Option<NaiveDate>,
    pub amount_cents: i64,
    /// Original bank label, never modified.
    pub raw_label: String,
    /// Cleaned label used for categorization and payees.
    pub normalized_label: String,
    /// Payee of the normalized label.
    pub payee_id: Option<i64>,
    /// Name of the payee, or the normalized label when the operation has no payee yet.
    pub payee_name: String,
    /// `None` for manual entries.
    pub import_batch_id: Option<i64>,
    /// The file of the import was kept: it can be opened (imports made since V2).
    pub has_source_document: bool,
    /// Page of the operation in its PDF statement, from 1.
    pub source_page: Option<u32>,
    /// Line of the operation in its text file (CSV, OFX, QIF), from 1.
    pub source_line: Option<u32>,
    /// `None` while the operation is still to categorize, and when it is split.
    pub category_id: Option<i64>,
    /// Parts of a split operation, in entry order; empty when it has a single category.
    pub splits: Vec<SplitLine>,
    /// Derived from the category; excluded from income and expense statistics.
    pub is_internal_transfer: bool,
    /// The other side of the internal transfer, on another account, once linked.
    pub transfer_pair_id: Option<i64>,
    pub note: Option<String>,
    /// Tag names, by name.
    pub tags: Vec<String>,
    pub attachment_count: i64,
}

/// An operation in a listing, with the balance of its account after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationRow {
    pub operation: Operation,
    pub balance_cents: i64,
}

/// A manual entry.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewOperation {
    pub account_id: i64,
    pub date: NaiveDate,
    pub value_date: Option<NaiveDate>,
    pub amount_cents: i64,
    pub label: String,
    /// Ignored when `splits` is not empty.
    pub category_id: Option<i64>,
    /// Parts of a split operation; empty for a single category.
    pub splits: Vec<SplitLine>,
    pub note: Option<String>,
    /// Tag names; unknown ones create a tag.
    pub tags: Vec<String>,
}

/// New values of an operation. For an imported operation, `date`, `amount_cents` and `label`
/// must be left unchanged: only the value date, the category, the note and the tags can be
/// edited.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationUpdate {
    pub date: NaiveDate,
    pub value_date: Option<NaiveDate>,
    pub amount_cents: i64,
    pub label: String,
    /// Ignored when `splits` is not empty.
    pub category_id: Option<i64>,
    /// Parts of a split operation; empty for a single category.
    pub splits: Vec<SplitLine>,
    pub note: Option<String>,
    /// Tag names; unknown ones create a tag.
    pub tags: Vec<String>,
}

/// A part of a split operation: at least two parts, each with a category other than the
/// internal transfer one and an amount of the sign of the operation, summing to its amount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SplitLine {
    pub category_id: i64,
    pub amount_cents: i64,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OperationError {
    #[error("operation {0} not found")]
    NotFound(i64),

    #[error("category {0} not found")]
    CategoryNotFound(i64),

    #[error("operation label cannot be empty")]
    EmptyLabel,

    #[error("operation amount cannot be zero")]
    ZeroAmount,

    #[error("investment, asset and liability accounts have no operations")]
    AccountWithoutOperations,

    #[error("operation date cannot precede the account opening date ({0})")]
    DateBeforeOpening(NaiveDate),

    #[error("the date, amount and label of an imported operation cannot be changed")]
    ImportedFieldsLocked,

    #[error("an imported operation cannot be deleted alone, cancel its import instead")]
    ImportedOperationNotDeletable,

    #[error("a split operation needs at least two parts")]
    SplitTooFewParts,

    #[error("a part of a split operation cannot be an internal transfer")]
    SplitInternalTransfer,

    #[error("each part of a split operation needs a non-zero amount of the operation's sign")]
    SplitPartAmount,

    #[error("the parts of a split operation sum to {actual}, not {expected}")]
    SplitTotalMismatch { expected: i64, actual: i64 },
}

impl OperationError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            OperationError::NotFound(_) | OperationError::CategoryNotFound(_) => {
                ErrorKind::NotFound
            }
            _ => ErrorKind::Validation,
        }
    }
}
