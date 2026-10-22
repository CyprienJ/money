use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use crate::currency::{self, CurrencyUse, ExchangeRate};
use crate::error::AppResult;

/// Supported currencies, euros (the reference) first.
#[tauri::command]
#[specta::specta]
pub fn supported_currencies() -> Vec<String> {
    currency::CURRENCIES
        .iter()
        .map(|c| (*c).to_owned())
        .collect()
}

/// Currencies other than euros used by the accounts and securities, with their latest rate.
#[tauri::command]
#[specta::specta]
pub async fn currencies_in_use(pool: State<'_, SqlitePool>) -> AppResult<Vec<CurrencyUse>> {
    currency::in_use(&pool).await
}

/// Rates of a currency, the most recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_exchange_rates(
    pool: State<'_, SqlitePool>,
    currency: String,
) -> AppResult<Vec<ExchangeRate>> {
    currency::list_rates(&pool, &currency).await
}

/// Records the rate of a day by hand (euros for one unit, in billionths); fetched rates never
/// overwrite it.
#[tauri::command]
#[specta::specta]
pub async fn set_exchange_rate(
    pool: State<'_, SqlitePool>,
    currency: String,
    date: NaiveDate,
    rate_nano: i64,
) -> AppResult<()> {
    currency::set_manual_rate(&pool, &currency, date, rate_nano).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_exchange_rate(
    pool: State<'_, SqlitePool>,
    currency: String,
    date: NaiveDate,
) -> AppResult<()> {
    currency::delete_rate(&pool, &currency, date).await
}
