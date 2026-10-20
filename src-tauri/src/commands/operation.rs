use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::operation::{self, NewOperation, Operation, OperationRow, OperationUpdate};

/// Operations of an account, most recent first, with the balance after each one.
#[tauri::command]
#[specta::specta]
pub async fn list_operations(
    pool: State<'_, SqlitePool>,
    account_id: i64,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> AppResult<Vec<OperationRow>> {
    operation::list(&pool, account_id, from, to).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_operation(pool: State<'_, SqlitePool>, id: i64) -> AppResult<Operation> {
    operation::get(&pool, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_operation(
    pool: State<'_, SqlitePool>,
    operation: NewOperation,
) -> AppResult<Operation> {
    operation::create(&pool, operation).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_operation(
    pool: State<'_, SqlitePool>,
    id: i64,
    changes: OperationUpdate,
) -> AppResult<Operation> {
    operation::update(&pool, id, changes).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_operation_category(
    pool: State<'_, SqlitePool>,
    id: i64,
    category_id: Option<i64>,
) -> AppResult<Operation> {
    operation::set_category(&pool, id, category_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_operation(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    operation::delete(&pool, id).await
}
