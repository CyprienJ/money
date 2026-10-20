use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::operation::category::{self, Category, CategoryInput};
use crate::operation::schedule::{
    self, ScheduledOperation, ScheduledOperationInput, UpcomingOperation,
};
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

#[tauri::command]
#[specta::specta]
pub async fn list_categories(pool: State<'_, SqlitePool>) -> AppResult<Vec<Category>> {
    category::list(&pool).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_category(
    pool: State<'_, SqlitePool>,
    category: CategoryInput,
) -> AppResult<Category> {
    category::create(&pool, category).await
}

/// Renames, moves or recolors a category; subcategories follow their top-level category.
#[tauri::command]
#[specta::specta]
pub async fn update_category(
    pool: State<'_, SqlitePool>,
    id: i64,
    changes: CategoryInput,
) -> AppResult<Category> {
    category::update(&pool, id, changes).await
}

/// Deletes a category without subcategories; its operations move to `reassign_to`, or become
/// uncategorized with `null`.
#[tauri::command]
#[specta::specta]
pub async fn delete_category(
    pool: State<'_, SqlitePool>,
    id: i64,
    reassign_to: Option<i64>,
) -> AppResult<()> {
    category::delete(&pool, id, reassign_to).await
}

/// Scheduled operations, by label.
#[tauri::command]
#[specta::specta]
pub async fn list_scheduled_operations(
    pool: State<'_, SqlitePool>,
) -> AppResult<Vec<ScheduledOperation>> {
    schedule::list(&pool).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_scheduled_operation(
    pool: State<'_, SqlitePool>,
    schedule: ScheduledOperationInput,
) -> AppResult<ScheduledOperation> {
    schedule::create(&pool, schedule).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_scheduled_operation(
    pool: State<'_, SqlitePool>,
    id: i64,
    schedule: ScheduledOperationInput,
) -> AppResult<ScheduledOperation> {
    schedule::update(&pool, id, schedule).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_scheduled_operation(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    schedule::delete(&pool, id).await
}

/// Occurrences of the scheduled operations over the next `days` days, today included. No
/// operation is created.
#[tauri::command]
#[specta::specta]
pub async fn upcoming_operations(
    pool: State<'_, SqlitePool>,
    days: u32,
) -> AppResult<Vec<UpcomingOperation>> {
    let today = chrono::Local::now().date_naive();
    schedule::upcoming(&pool, today, days).await
}
