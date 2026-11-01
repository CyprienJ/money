//! Currencies (V2): each account has a currency, taken by its operations, and each security
//! the currency of its prices. Statistics convert every amount to euros, the reference
//! currency, with the exchange rates entered by hand or fetched with the security prices.
//!
//! A flow (income, expense) is converted at the rate of its day, a balance or a value at the
//! rate of the day of the balance: the last rate on or before that day, else the first one
//! after. An amount in a currency without any rate counts 0 in the totals, and raises an alert.

mod rate;
#[cfg(test)]
mod tests;

pub(crate) use rate::update_rates;
pub use rate::{CurrencyUse, ExchangeRate, delete_rate, in_use, list_rates, set_manual_rate};

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use crate::amount::mul_div;
use crate::error::{AppResult, ErrorKind};

/// Currency of the statistics.
pub const REFERENCE: &str = "EUR";

/// Supported currencies: their amounts have two decimals, stored in hundredths.
pub const CURRENCIES: [&str; 14] = [
    "EUR", "USD", "GBP", "CHF", "CAD", "AUD", "NZD", "SEK", "NOK", "DKK", "PLN", "CZK", "HKD",
    "SGD",
];

/// Billionths in a unit: a rate is stored as euros for one unit of the currency × this.
pub const RATE_SCALE: i64 = 1_000_000_000;

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CurrencyError {
    #[error("the currency {0:?} is not supported")]
    Unsupported(String),

    #[error("euros are the reference currency: they have no exchange rate")]
    ReferenceRate,

    #[error("an exchange rate must be positive")]
    InvalidRate,

    #[error("the account has data: its currency can no longer change")]
    AccountCurrencyLocked,

    #[error("the security has prices: its currency can no longer change")]
    SecurityCurrencyLocked,
}

impl CurrencyError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::Validation
    }
}

/// The code of a supported currency, trimmed and in capitals.
pub fn check_supported(code: &str) -> Result<String, CurrencyError> {
    let code = code.trim().to_uppercase();
    if CURRENCIES.contains(&code.as_str()) {
        Ok(code)
    } else {
        Err(CurrencyError::Unsupported(code))
    }
}

/// Exchange rates, loaded once to convert many amounts.
#[derive(Debug, Clone, Default)]
pub struct Rates {
    /// (date, rate in billionths) of each currency, by date.
    by_currency: HashMap<String, Vec<(NaiveDate, i64)>>,
}

impl Rates {
    pub async fn load(pool: &SqlitePool) -> AppResult<Rates> {
        let rows = sqlx::query!(
            r#"SELECT currency, date AS "date: NaiveDate", rate_nano
               FROM exchange_rate
               ORDER BY currency, date"#
        )
        .fetch_all(pool)
        .await?;
        let mut by_currency: HashMap<String, Vec<(NaiveDate, i64)>> = HashMap::new();
        for row in rows {
            by_currency
                .entry(row.currency)
                .or_default()
                .push((row.date, row.rate_nano));
        }
        Ok(Rates { by_currency })
    }

    /// Euros for one unit of `currency` on `date`, in billionths: the last rate on or before
    /// `date`, else the first one after; `None` without any rate.
    pub fn rate(&self, currency: &str, date: NaiveDate) -> Option<i64> {
        if currency == REFERENCE {
            return Some(RATE_SCALE);
        }
        let rates = self.by_currency.get(currency)?;
        let count = rates.partition_point(|(day, _)| *day <= date);
        let index = count.saturating_sub(1);
        rates.get(index).map(|(_, rate)| *rate)
    }

    /// Amount in `from` converted to `to` on `date`, rounded to the cent.
    pub fn convert(&self, amount_cents: i64, from: &str, to: &str, date: NaiveDate) -> Option<i64> {
        if from == to {
            return Some(amount_cents);
        }
        Some(mul_div(
            amount_cents,
            self.rate(from, date)?,
            self.rate(to, date)?,
        ))
    }

    /// Amount in euros on `date`; 0 without a rate for `currency`.
    pub fn to_reference(&self, amount_cents: i64, currency: &str, date: NaiveDate) -> i64 {
        self.convert(amount_cents, currency, REFERENCE, date)
            .unwrap_or(0)
    }
}
