use chrono::{Days, NaiveDate};
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::stats::{
    self, BalanceForecast, BalancePoint, BalanceStep, CategoryComparison, CategoryExpense,
    CategoryLevel, DateRange, Granularity, LargestExpense, NetWorth, PayeeExpense,
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

/// Projected balance from today over `days` days, from the schedule occurrences not realized
/// yet. `accounts`: `null` for every account with operations.
#[tauri::command]
#[specta::specta]
pub async fn balance_forecast(
    pool: State<'_, SqlitePool>,
    days: u32,
    accounts: Option<Vec<i64>>,
) -> AppResult<BalanceForecast> {
    let today = super::operation::today();
    let to = today
        .checked_add_days(Days::new(days.into()))
        .unwrap_or(today);
    stats::balance_forecast(&pool, today, to, accounts.as_deref()).await
}

/// Net worth at the end of `date`, with its breakdown (cash, investments, assets, debts).
#[tauri::command]
#[specta::specta]
pub async fn net_worth_at(pool: State<'_, SqlitePool>, date: NaiveDate) -> AppResult<NetWorth> {
    stats::net_worth_at(&pool, date).await
}

/// Net worth at the end of each day, week or month between `from` and `to`.
#[tauri::command]
#[specta::specta]
pub async fn net_worth_history(
    pool: State<'_, SqlitePool>,
    from: NaiveDate,
    to: NaiveDate,
    step: BalanceStep,
) -> AppResult<Vec<NetWorth>> {
    stats::net_worth_history(&pool, from, to, step).await
}

/// Net expenses by category of `current` and `previous`, internal transfers excluded.
#[tauri::command]
#[specta::specta]
pub async fn compare_expenses_by_category(
    pool: State<'_, SqlitePool>,
    current: DateRange,
    previous: DateRange,
    accounts: Option<Vec<i64>>,
    level: CategoryLevel,
) -> AppResult<Vec<CategoryComparison>> {
    stats::compare_expenses_by_category(&pool, current, previous, accounts.as_deref(), level).await
}

/// The `limit` payees with the largest net expenses between `from` and `to`.
#[tauri::command]
#[specta::specta]
pub async fn top_payees(
    pool: State<'_, SqlitePool>,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<Vec<i64>>,
    limit: u32,
) -> AppResult<Vec<PayeeExpense>> {
    stats::top_payees(&pool, from, to, accounts.as_deref(), limit).await
}

/// The `limit` largest single expenses between `from` and `to`.
#[tauri::command]
#[specta::specta]
pub async fn largest_expenses(
    pool: State<'_, SqlitePool>,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<Vec<i64>>,
    limit: u32,
) -> AppResult<Vec<LargestExpense>> {
    stats::largest_expenses(&pool, from, to, accounts.as_deref(), limit).await
}
