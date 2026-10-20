//! Operations, splits and categories.

pub mod category;
mod repository;
pub mod schedule;
#[cfg(test)]
mod tests;

pub(crate) use repository::{check_account, internal_transfer_flag, timestamp};
pub use repository::{create, delete, get, list, set_category, update};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::ErrorKind;

/// An operation with its category. V1 has exactly one split per operation.
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
    /// `None` for manual entries.
    pub import_batch_id: Option<i64>,
    /// `None` while the operation is still to categorize.
    pub category_id: Option<i64>,
    /// Derived from the category; excluded from income and expense statistics.
    pub is_internal_transfer: bool,
}

/// An operation in an account listing, with the account balance after it.
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
    pub category_id: Option<i64>,
}

/// New values of an operation. For an imported operation, `date`, `amount_cents` and `label`
/// must be left unchanged: only the value date and the category can be edited.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationUpdate {
    pub date: NaiveDate,
    pub value_date: Option<NaiveDate>,
    pub amount_cents: i64,
    pub label: String,
    pub category_id: Option<i64>,
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
