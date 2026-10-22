//! Detailed portfolio of investment accounts: securities, and buys, sells and dividends
//! entered by hand.
//!
//! Quantities are in millionths of a unit (fractional shares), amounts in cents. The cost of a
//! position is the weighted average cost, buying fees included (French tax rule): a buy adds
//! its amount and fees, a sell removes the share of the cost of the units sold, leaving the
//! unit cost unchanged. The value of the account and the net worth still come from its
//! valuations.

mod position;
mod security;
#[cfg(test)]
mod tests;
mod transaction;

pub use position::{Position, positions};
pub use security::{Security, SecurityInput};
pub use transaction::{InvestmentTransaction, TransactionInput, TransactionKind};

pub mod securities {
    pub use super::security::{create, delete, list, update};
}
pub mod transactions {
    pub use super::transaction::{create, delete, list, update};
}

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;

use crate::error::ErrorKind;

/// Millionths of a unit in a unit.
pub const QUANTITY_SCALE: i64 = 1_000_000;

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InvestmentError {
    #[error("security {0} not found")]
    SecurityNotFound(i64),

    #[error("investment transaction {0} not found")]
    TransactionNotFound(i64),

    #[error("the name of the security cannot be empty")]
    EmptySecurityName,

    #[error("{0:?} is not a valid ISIN")]
    InvalidIsin(String),

    #[error("a security with the ISIN {0} already exists")]
    IsinTaken(String),

    #[error("the security has transactions and cannot be deleted")]
    SecurityInUse,

    #[error("only investment accounts have a portfolio")]
    NotAnInvestmentAccount,

    #[error("the date precedes the opening of the account ({0})")]
    DateBeforeOpening(NaiveDate),

    #[error("a buy or a sell needs a quantity")]
    MissingQuantity,

    #[error("a dividend has no quantity")]
    QuantityOnDividend,

    #[error("the amount must be positive")]
    ZeroAmount,

    #[error("on {date}, more {security} would be sold than held")]
    Oversold { date: NaiveDate, security: String },
}

impl InvestmentError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            InvestmentError::SecurityNotFound(_) | InvestmentError::TransactionNotFound(_) => {
                ErrorKind::NotFound
            }
            _ => ErrorKind::Validation,
        }
    }
}
