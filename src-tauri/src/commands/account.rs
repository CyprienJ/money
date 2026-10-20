use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use crate::account::valuation::{self, Valuation, ValuationInput};
use crate::account::{self, Account, AccountSummary, AccountUpdate, NewAccount};
use crate::error::AppResult;

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(
    pool: State<'_, SqlitePool>,
    include_archived: bool,
) -> AppResult<Vec<Account>> {
    account::list(&pool, include_archived).await
}

/// Accounts with their balance as of today (local date).
#[tauri::command]
#[specta::specta]
pub async fn list_account_summaries(
    pool: State<'_, SqlitePool>,
    include_archived: bool,
) -> AppResult<Vec<AccountSummary>> {
    let today = chrono::Local::now().date_naive();
    account::list_summaries(&pool, include_archived, today).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_account(pool: State<'_, SqlitePool>, id: i64) -> AppResult<Account> {
    account::get(&pool, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_account(
    pool: State<'_, SqlitePool>,
    account: NewAccount,
) -> AppResult<Account> {
    account::create(&pool, account).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_account(
    pool: State<'_, SqlitePool>,
    id: i64,
    changes: AccountUpdate,
) -> AppResult<Account> {
    account::update(&pool, id, changes).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_account_archived(
    pool: State<'_, SqlitePool>,
    id: i64,
    archived: bool,
) -> AppResult<Account> {
    account::set_archived(&pool, id, archived).await
}

/// Balance in cents at the end of `date`, for accounts with operations.
#[tauri::command]
#[specta::specta]
pub async fn account_balance(
    pool: State<'_, SqlitePool>,
    id: i64,
    date: NaiveDate,
) -> AppResult<i64> {
    account::balance_at(&pool, id, date).await
}

/// Valuations of an investment, asset or liability account, the most recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_valuations(
    pool: State<'_, SqlitePool>,
    account_id: i64,
) -> AppResult<Vec<Valuation>> {
    valuation::list(&pool, account_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_valuation(
    pool: State<'_, SqlitePool>,
    account_id: i64,
    valuation: ValuationInput,
) -> AppResult<Valuation> {
    valuation::create(&pool, account_id, valuation).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_valuation(
    pool: State<'_, SqlitePool>,
    id: i64,
    valuation: ValuationInput,
) -> AppResult<Valuation> {
    valuation::update(&pool, id, valuation).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_valuation(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    valuation::delete(&pool, id).await
}
