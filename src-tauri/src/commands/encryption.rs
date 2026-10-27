use std::path::PathBuf;

use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};

use super::app::data_dir;
use crate::db::{DATA_FILE_NAME, encryption};
use crate::error::AppResult;
use crate::startup;

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LockStatus {
    /// The data file is encrypted.
    pub encrypted: bool,
    /// The data is open: false until the password of an encrypted file is given.
    pub unlocked: bool,
}

fn data_file(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join(DATA_FILE_NAME))
}

#[tauri::command]
#[specta::specta]
pub fn lock_status(app: AppHandle) -> AppResult<LockStatus> {
    Ok(LockStatus {
        encrypted: encryption::is_encrypted(&data_file(&app)?)?,
        unlocked: app.try_state::<SqlitePool>().is_some(),
    })
}

/// Opens the encrypted data file with its password.
#[tauri::command]
#[specta::specta]
pub async fn unlock(app: AppHandle, password: String) -> AppResult<()> {
    if app.try_state::<SqlitePool>().is_some() {
        return Ok(());
    }
    let pool = encryption::open(&data_file(&app)?, &password).await?;
    startup::start(&app, pool).await
}

/// Encrypts the data file with a password, then restarts the application.
#[tauri::command]
#[specta::specta]
pub async fn encrypt_data(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    password: String,
) -> AppResult<()> {
    encryption::encrypt(&pool, &data_file(&app)?, &password).await?;
    app.restart()
}

/// Encrypts the data file with a new password, then restarts the application.
#[tauri::command]
#[specta::specta]
pub async fn change_data_password(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    current_password: String,
    new_password: String,
) -> AppResult<()> {
    encryption::change_password(&pool, &data_file(&app)?, &current_password, &new_password).await?;
    app.restart()
}

/// Decrypts the data file, then restarts the application.
#[tauri::command]
#[specta::specta]
pub async fn decrypt_data(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    current_password: String,
) -> AppResult<()> {
    encryption::decrypt(&pool, &data_file(&app)?, &current_password).await?;
    app.restart()
}
