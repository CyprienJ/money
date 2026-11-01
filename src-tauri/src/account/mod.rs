//! Accounts, valuations and net worth.

mod balance;
mod repository;
pub mod valuation;

pub use balance::{AccountSummary, balance_at, list_summaries};
pub use repository::{create, get, list, set_archived, update};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::ErrorKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountType {
    Checking,
    Savings,
    /// Deferred debit card.
    CreditCard,
    /// PEA, CTO, life insurance: only the total value is tracked in V1.
    Investment,
    /// Real estate, vehicle… valued manually.
    Asset,
    /// Loan, tracked by its outstanding capital.
    Liability,
}

impl AccountType {
    /// Accounts whose balance comes from their operations. The others are tracked by valuations.
    pub fn has_operations(self) -> bool {
        matches!(
            self,
            AccountType::Checking | AccountType::Savings | AccountType::CreditCard
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: AccountType,
    pub currency: String,
    pub is_joint: bool,
    pub institution: Option<String>,
    pub opening_balance_cents: i64,
    pub opening_date: NaiveDate,
    pub archived: bool,
}

/// Fields chosen when creating an account.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewAccount {
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: AccountType,
    pub currency: String,
    pub is_joint: bool,
    pub institution: Option<String>,
    pub opening_balance_cents: i64,
    pub opening_date: NaiveDate,
}

/// Editable fields of an existing account. The type is fixed at creation, the currency once
/// the account has data.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdate {
    pub name: String,
    pub currency: String,
    pub is_joint: bool,
    pub institution: Option<String>,
    pub opening_balance_cents: i64,
    pub opening_date: NaiveDate,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AccountError {
    #[error("account {0} not found")]
    NotFound(i64),

    #[error("account name cannot be empty")]
    EmptyName,

    #[error("valuation {0} not found")]
    ValuationNotFound(i64),

    #[error(
        "checking, savings and credit card accounts are tracked by their operations, not valuations"
    )]
    NotValued,

    #[error("a valuation cannot be negative (a debt is valued by its outstanding capital)")]
    NegativeValuation,

    #[error("the account already has a valuation on {0}")]
    DuplicateValuationDate(NaiveDate),

    #[error("a valuation cannot precede the account opening date ({0})")]
    ValuationBeforeOpening(NaiveDate),
}

impl AccountError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            AccountError::NotFound(_) | AccountError::ValuationNotFound(_) => ErrorKind::NotFound,
            _ => ErrorKind::Validation,
        }
    }
}

/// Personal share, rounded to the cent with ties away from zero.
pub fn personal_share(cents: i64, is_joint: bool) -> i64 {
    crate::amount::mul_div(cents, 1, if is_joint { 2 } else { 1 })
}

#[cfg(test)]
mod ownership_tests {
    use super::personal_share;

    #[test]
    fn shares_round_symmetrically_without_overflow() {
        for (whole, half) in [
            (0, 0),
            (1, 1),
            (-1, -1),
            (3, 2),
            (-3, -2),
            (4, 2),
            (i64::MAX, 4_611_686_018_427_387_904),
            (i64::MIN, -4_611_686_018_427_387_904),
        ] {
            assert_eq!(personal_share(whole, false), whole);
            assert_eq!(personal_share(whole, true), half);
        }
    }
}
