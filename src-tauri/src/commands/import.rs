use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::importing::batch::{self, ImportBatch};

/// Imports of an account, most recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_import_batches(
    pool: State<'_, SqlitePool>,
    account_id: i64,
) -> AppResult<Vec<ImportBatch>> {
    batch::list(&pool, account_id).await
}

/// Deletes an import and all its operations.
#[tauri::command]
#[specta::specta]
pub async fn cancel_import(pool: State<'_, SqlitePool>, id: i64) -> AppResult<()> {
    batch::cancel(&pool, id).await
}
