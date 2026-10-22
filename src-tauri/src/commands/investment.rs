use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use super::operation::today;
use crate::error::AppResult;
use crate::investment::prices::{self, PriceSettings, PriceUpdate, SecurityPrice, SymbolCandidate};
use crate::investment::yahoo::Yahoo;
use crate::investment::{
    self, InvestmentTransaction, Position, Security, SecurityInput, TransactionInput, securities,
    transactions,
};

/// Every security, by name, with its count of transactions.
#[tauri::command]
#[specta::specta]
pub async fn list_securities(pool: State<'_, SqlitePool>) -> AppResult<Vec<Security>> {
    securities::list(&pool).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_security(
    pool: State<'_, SqlitePool>,
    input: SecurityInput,
) -> AppResult<Security> {
    securities::create(&pool, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_security(
    pool: State<'_, SqlitePool>,
    id: i64,
    input: SecurityInput,
) -> AppResult<Security> {
    securities::update(&pool, id, input).await
}

/// Deletes a security without transactions.
#[tauri::command]
#[specta::specta]
pub async fn delete_security(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    securities::delete(&pool, id).await
}

/// Transactions of an investment account, the most recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_investment_transactions(
    pool: State<'_, SqlitePool>,
    account_id: i64,
) -> AppResult<Vec<InvestmentTransaction>> {
    transactions::list(&pool, account_id).await
}

/// Records a buy, a sell or a dividend; refused when a sell would exceed the units held.
#[tauri::command]
#[specta::specta]
pub async fn create_investment_transaction(
    pool: State<'_, SqlitePool>,
    account_id: i64,
    input: TransactionInput,
) -> AppResult<InvestmentTransaction> {
    transactions::create(&pool, account_id, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_investment_transaction(
    pool: State<'_, SqlitePool>,
    id: i64,
    input: TransactionInput,
) -> AppResult<InvestmentTransaction> {
    transactions::update(&pool, id, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_investment_transaction(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    transactions::delete(&pool, id).await
}

/// Securities held in an investment account at the end of `date`, with their cost.
#[tauri::command]
#[specta::specta]
pub async fn portfolio_positions(
    pool: State<'_, SqlitePool>,
    account_id: i64,
    date: NaiveDate,
) -> AppResult<Vec<Position>> {
    investment::positions(&pool, account_id, date).await
}

/// Whether prices are fetched from Yahoo Finance, and the day of the last update.
#[tauri::command]
#[specta::specta]
pub async fn price_settings(pool: State<'_, SqlitePool>) -> AppResult<PriceSettings> {
    prices::settings(&pool).await
}

/// Allows or stops fetching prices from Yahoo Finance, the only network access.
#[tauri::command]
#[specta::specta]
pub async fn set_price_updates_enabled(
    pool: State<'_, SqlitePool>,
    enabled: bool,
) -> AppResult<PriceSettings> {
    prices::set_updates_enabled(&pool, enabled).await
}

/// Fetches the recent prices of the securities with a symbol.
#[tauri::command]
#[specta::specta]
pub async fn update_security_prices(pool: State<'_, SqlitePool>) -> AppResult<PriceUpdate> {
    prices::update(&pool, &Yahoo::new()?, today()).await
}

/// Listings of a security found on Yahoo Finance by its ISIN.
#[tauri::command]
#[specta::specta]
pub async fn search_security_symbols(
    pool: State<'_, SqlitePool>,
    isin: String,
) -> AppResult<Vec<SymbolCandidate>> {
    prices::search_symbols(&pool, &Yahoo::new()?, &isin).await
}

/// Prices of a security, the most recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_security_prices(
    pool: State<'_, SqlitePool>,
    security_id: i64,
) -> AppResult<Vec<SecurityPrice>> {
    prices::list(&pool, security_id).await
}

/// Records the price of a day by hand; fetched prices never overwrite it.
#[tauri::command]
#[specta::specta]
pub async fn set_security_price(
    pool: State<'_, SqlitePool>,
    security_id: i64,
    date: NaiveDate,
    price_cents: i64,
) -> AppResult<()> {
    prices::set_manual(&pool, security_id, date, price_cents).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_security_price(
    pool: State<'_, SqlitePool>,
    security_id: i64,
    date: NaiveDate,
) -> AppResult<()> {
    prices::delete(&pool, security_id, date).await
}
