//! Accounts, valuations and net worth.

mod balance;
mod repository;

pub use balance::{AccountSummary, balance_at, list_summaries};
pub use repository::{create, get, list, set_archived, update};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;

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
    pub institution: Option<String>,
    pub opening_balance_cents: i64,
    pub opening_date: NaiveDate,
}

/// Editable fields of an existing account. The type is fixed at creation.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdate {
    pub name: String,
    pub institution: Option<String>,
    pub opening_balance_cents: i64,
    pub opening_date: NaiveDate,
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("account {0} not found")]
    NotFound(i64),

    #[error("account name cannot be empty")]
    EmptyName,
}
