use std::path::PathBuf;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::db::DATA_FILE_NAME;
use crate::error::{AppError, AppResult};
use crate::importing::batch;

/// Version of the application, as declared in `tauri.conf.json`.
#[tauri::command]
#[specta::specta]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Where the application keeps its data, for the Settings screen.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DataLocation {
    /// The SQLite data file.
    pub data_file: String,
    /// Directory of the automatic backups made before each import.
    pub backups_dir: String,
    pub backup_count: u32,
}

#[tauri::command]
#[specta::specta]
pub fn data_location(app: AppHandle) -> AppResult<DataLocation> {
    let backups = backups_dir(&app)?;
    Ok(DataLocation {
        data_file: data_dir(&app)?.join(DATA_FILE_NAME).display().to_string(),
        backups_dir: backups.display().to_string(),
        backup_count: u32::try_from(batch::backup_count(&backups)?).unwrap_or(u32::MAX),
    })
}

/// `app_data_dir()` of Tauri (README, data location).
pub(super) fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|error| AppError::Io(std::io::Error::other(error)))
}

/// `backups/` in the application data directory.
pub(super) fn backups_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join("backups"))
}

/// `statements/` in the application data directory: the kept imported files.
pub(super) fn statements_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join(crate::importing::source_document::STATEMENTS_DIR))
}

/// `attachments/` in the application data directory.
pub(super) fn attachments_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join(crate::attachment::ATTACHMENTS_DIR))
}
