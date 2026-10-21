use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::labeling::rules::{self, Rule, RuleInput};
use crate::operation::category::{self, Category, CategoryInput};
use crate::operation::same_label::{self, SameLabel};
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

/// Other operations sharing the label of an operation: those still to categorize, and those
/// in another category.
#[tauri::command]
#[specta::specta]
pub async fn same_label_operations(pool: State<'_, SqlitePool>, id: i64) -> AppResult<SameLabel> {
    same_label::same_label(&pool, id).await
}

/// Gives the category of an operation to the others of its label still to categorize;
/// returns their count.
#[tauri::command]
#[specta::specta]
pub async fn categorize_same_label(pool: State<'_, SqlitePool>, id: i64) -> AppResult<u32> {
    same_label::categorize_same_label(&pool, id).await
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

/// Categorization rules, in order, each with the count of operations to categorize it matches.
#[tauri::command]
#[specta::specta]
pub async fn list_rules(pool: State<'_, SqlitePool>) -> AppResult<Vec<Rule>> {
    rules::list(&pool).await
}

/// Operations to categorize whose label contains `pattern`, to preview a rule.
#[tauri::command]
#[specta::specta]
pub async fn count_rule_matches(pool: State<'_, SqlitePool>, pattern: String) -> AppResult<u32> {
    rules::count_pending(&pool, &pattern).await
}

/// Adds a rule at the end of the list.
#[tauri::command]
#[specta::specta]
pub async fn create_rule(pool: State<'_, SqlitePool>, rule: RuleInput) -> AppResult<Rule> {
    rules::create(&pool, rule).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_rule(pool: State<'_, SqlitePool>, id: i64, rule: RuleInput) -> AppResult<Rule> {
    rules::update(&pool, id, rule).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_rule(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    rules::delete(&pool, id).await
}

/// Puts the rules in the order of `ids`: the first matching rule wins.
#[tauri::command]
#[specta::specta]
pub async fn reorder_rules(pool: State<'_, SqlitePool>, ids: Vec<i64>) -> AppResult<()> {
    rules::reorder(&pool, &ids).await
}

/// Gives the category of a rule to the operations to categorize it matches; returns their count.
#[tauri::command]
#[specta::specta]
pub async fn apply_rule(pool: State<'_, SqlitePool>, id: i64) -> AppResult<u32> {
    rules::apply(&pool, id).await
}
