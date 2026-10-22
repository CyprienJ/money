use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
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
