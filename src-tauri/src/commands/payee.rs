use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::payee::{self, Payee};

/// Payees that have operations, by name, with their labels.
#[tauri::command]
#[specta::specta]
pub async fn list_payees(pool: State<'_, SqlitePool>) -> AppResult<Vec<Payee>> {
    payee::list(&pool).await
}

/// Renames a payee; refused when another payee already has this name.
#[tauri::command]
#[specta::specta]
pub async fn rename_payee(pool: State<'_, SqlitePool>, id: i64, name: String) -> AppResult<Payee> {
    payee::rename(&pool, id, &name).await
}

/// Merges the `sources` payees into `target`, which keeps its name.
#[tauri::command]
#[specta::specta]
pub async fn merge_payees(
    pool: State<'_, SqlitePool>,
    target: i64,
    sources: Vec<i64>,
) -> AppResult<Payee> {
    payee::merge(&pool, target, &sources).await
}

/// Detaches a label from its payee into a new payee; returns the new payee.
#[tauri::command]
#[specta::specta]
pub async fn detach_payee_label(
    pool: State<'_, SqlitePool>,
    normalized_label: String,
) -> AppResult<Payee> {
    payee::detach(&pool, &normalized_label).await
}
