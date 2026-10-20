use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::stats::{
    self, BalancePoint, BalanceStep, CategoryExpense, CategoryLevel, Granularity,
    PeriodIncomeExpenses,
};

/// Net expenses by category between `from` and `to` (included), internal transfers excluded.
/// `accounts`: `null` for every account with operations.
#[tauri::command]
#[specta::specta]
pub async fn expenses_by_category(
    pool: State<'_, SqlitePool>,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<Vec<i64>>,
    level: CategoryLevel,
) -> AppResult<Vec<CategoryExpense>> {
    stats::expenses_by_category(&pool, from, to, accounts.as_deref(), level).await
}

/// Income and expenses by month or year, internal transfers excluded.
#[tauri::command]
#[specta::specta]
pub async fn income_vs_expenses(
    pool: State<'_, SqlitePool>,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<Vec<i64>>,
    granularity: Granularity,
) -> AppResult<Vec<PeriodIncomeExpenses>> {
    stats::income_vs_expenses(&pool, from, to, accounts.as_deref(), granularity).await
}

/// Total balance of the accounts at the end of each day, week or month.
#[tauri::command]
#[specta::specta]
pub async fn balance_history(
    pool: State<'_, SqlitePool>,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<Vec<i64>>,
    step: BalanceStep,
) -> AppResult<Vec<BalancePoint>> {
    stats::balance_history(&pool, from, to, accounts.as_deref(), step).await
}
