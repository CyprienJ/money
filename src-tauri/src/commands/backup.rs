use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use super::app::data_dir;
use crate::backup::{self, Backup, BackupSettings, Dirs};
use crate::error::AppResult;

fn dirs(app: &AppHandle) -> AppResult<Dirs> {
    Ok(Dirs::new(data_dir(app)?))
}

/// Daily full backups, their folder and the day of the last one.
#[tauri::command]
#[specta::specta]
pub async fn backup_settings(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
) -> AppResult<BackupSettings> {
    backup::settings(&pool, &dirs(&app)?).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_daily_backups(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    daily: bool,
) -> AppResult<BackupSettings> {
    backup::set_daily(&pool, daily).await?;
    backup::settings(&pool, &dirs(&app)?).await
}

/// Chooses the folder of the full backups; `null` goes back to the default one.
#[tauri::command]
#[specta::specta]
pub async fn set_backup_folder(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    folder: Option<String>,
) -> AppResult<BackupSettings> {
    backup::set_folder(&pool, folder.as_deref()).await?;
    backup::settings(&pool, &dirs(&app)?).await
}

/// Full backups and backups made before an import, the most recent first.
#[tauri::command]
#[specta::specta]
pub async fn list_backups(app: AppHandle, pool: State<'_, SqlitePool>) -> AppResult<Vec<Backup>> {
    backup::list(&pool, &dirs(&app)?).await
}

/// Makes a full backup now.
#[tauri::command]
#[specta::specta]
pub async fn create_full_backup(app: AppHandle, pool: State<'_, SqlitePool>) -> AppResult<Backup> {
    backup::full(&pool, &dirs(&app)?, true).await
}

/// Backs up the current data, then restarts the application to restore a listed backup.
#[tauri::command]
#[specta::specta]
pub async fn restore_backup(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    path: String,
) -> AppResult<()> {
    backup::schedule_restore(&pool, &dirs(&app)?, &path).await?;
    pool.close().await;
    app.restart()
}
