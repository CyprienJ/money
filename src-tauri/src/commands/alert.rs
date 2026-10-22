use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use super::operation::today;
use crate::alert::{self, Alert};
use crate::error::AppResult;

/// Alerts of today: negative forecasts, missing occurrences, unusual expenses.
#[tauri::command]
#[specta::specta]
pub async fn list_alerts(pool: State<'_, SqlitePool>) -> AppResult<Vec<Alert>> {
    alert::list(&pool, today()).await
}

/// Stops showing the unusual expense alert of a top-level category for the month of `month`.
#[tauri::command]
#[specta::specta]
pub async fn dismiss_unusual_expense(
    pool: State<'_, SqlitePool>,
    category_id: i64,
    month: NaiveDate,
) -> AppResult<()> {
    alert::dismiss_unusual_expense(&pool, category_id, month).await
}

/// Stops showing an occurrence of a scheduled operation as missing.
#[tauri::command]
#[specta::specta]
pub async fn dismiss_missing_occurrence(
    pool: State<'_, SqlitePool>,
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
) -> AppResult<()> {
    alert::dismiss_missing_occurrence(&pool, scheduled_operation_id, occurrence_date).await
}
