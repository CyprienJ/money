use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use crate::account::{self, Account, AccountUpdate, NewAccount};
use crate::error::AppResult;

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(
    pool: State<'_, SqlitePool>,
    include_archived: bool,
) -> AppResult<Vec<Account>> {
    account::list(&pool, include_archived).await
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
