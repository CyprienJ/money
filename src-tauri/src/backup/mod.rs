//! Full backups: a consistent copy of the data file (`VACUUM INTO`) and of the attachments, in
//! a `complete-AAAAMMJJ-HHMMSS/` directory of the backup folder. Made at startup once a day
//! (unless disabled) and on demand; the 10 most recent are kept. The backups made before each
//! import (data file only) are kept 20 by 20.
//!
//! A backup cannot replace the open data file: a restore is scheduled in a marker file of the
//! data directory, the application restarts, and the files are replaced before the data file
//! is opened.

use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate, NaiveDateTime};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use crate::attachment::ATTACHMENTS_DIR;
use crate::db::DATA_FILE_NAME;
use crate::error::{AppResult, ErrorKind};
use crate::setting;

#[cfg(test)]
mod tests;

/// Full backups kept.
pub const KEEP_FULL: usize = 10;
/// Backups made before an import kept.
pub const KEEP_IMPORT: usize = 20;

const FULL_PREFIX: &str = "complete-";
const IMPORT_PREFIX: &str = "money-";
const RESTORE_MARKER: &str = "restore-pending";

const ENABLED: &str = "full_backups_enabled";
const FOLDER: &str = "full_backup_folder";
const LAST_ON: &str = "full_backup_on";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BackupKind {
    /// Data file and attachments.
    Full,
    /// Data file only, made before an import.
    Import,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub path: String,
    pub kind: BackupKind,
    /// Local date and time of the backup, from its name.
    pub created_at: NaiveDateTime,
    /// Size of the data file and of the attachments.
    pub size_bytes: u64,
    pub attachment_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    /// A full backup is made at startup once a day.
    pub daily: bool,
    /// Folder of the full backups.
    pub folder: String,
    /// The folder was chosen by the user.
    pub custom_folder: bool,
    pub last_full_on: Option<NaiveDate>,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BackupError {
    #[error("{0} is not a backup of the application")]
    NotABackup(String),

    #[error("the folder {0} cannot be used for backups")]
    FolderUnavailable(String),
}

impl BackupError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::Validation
    }
}

/// Folders of the application data.
#[derive(Debug, Clone)]
pub struct Dirs {
    /// Application data directory: data file, attachments, restore marker.
    pub data: PathBuf,
    /// `backups/` of the data directory: backups made before an import, and full backups by
    /// default.
    pub default_backups: PathBuf,
}

impl Dirs {
    pub fn new(data: PathBuf) -> Dirs {
        let default_backups = data.join("backups");
        Dirs {
            data,
            default_backups,
        }
    }

    fn attachments(&self) -> PathBuf {
        self.data.join(ATTACHMENTS_DIR)
    }
}

pub async fn settings(pool: &SqlitePool, dirs: &Dirs) -> AppResult<BackupSettings> {
    let folder = setting::get(pool, FOLDER).await?;
    Ok(BackupSettings {
        daily: setting::get(pool, ENABLED).await?.as_deref() != Some("false"),
        custom_folder: folder.is_some(),
        folder: folder.unwrap_or_else(|| dirs.default_backups.display().to_string()),
        last_full_on: setting::get(pool, LAST_ON)
            .await?
            .and_then(|date| date.parse().ok()),
    })
}

pub async fn set_daily(pool: &SqlitePool, daily: bool) -> AppResult<()> {
    setting::set(pool, ENABLED, if daily { "true" } else { "false" }).await
}

/// Chooses the folder of the full backups; `None` goes back to the default one.
pub async fn set_folder(pool: &SqlitePool, folder: Option<&str>) -> AppResult<()> {
    match folder {
        None => {
            sqlx::query!("DELETE FROM app_setting WHERE key = ?1", FOLDER)
                .execute(pool)
                .await?;
        }
        Some(folder) => {
            let path = Path::new(folder);
            if std::fs::create_dir_all(path).is_err() || !is_writable(path) {
                return Err(BackupError::FolderUnavailable(folder.to_owned()).into());
            }
            setting::set(pool, FOLDER, folder).await?;
        }
    }
    Ok(())
}

fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".money-write-test");
    let written = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    written
}

async fn full_folder(pool: &SqlitePool, dirs: &Dirs) -> AppResult<PathBuf> {
    Ok(setting::get(pool, FOLDER)
        .await?
        .map_or_else(|| dirs.default_backups.clone(), PathBuf::from))
}

/// Makes a full backup, then deletes the oldest beyond `KEEP_FULL` when `rotate`.
pub async fn full(pool: &SqlitePool, dirs: &Dirs, rotate: bool) -> AppResult<Backup> {
    let folder = full_folder(pool, dirs).await?;
    std::fs::create_dir_all(&folder)
        .map_err(|_| BackupError::FolderUnavailable(folder.display().to_string()))?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut dir = folder.join(format!("{FULL_PREFIX}{stamp}"));
    let mut suffix = 1;
    while dir.exists() {
        suffix += 1;
        dir = folder.join(format!("{FULL_PREFIX}{stamp}-{suffix}"));
    }
    std::fs::create_dir(&dir)?;
    sqlx::query("VACUUM INTO ?1")
        .bind(dir.join(DATA_FILE_NAME).to_string_lossy().into_owned())
        .execute(pool)
        .await?;
    let (from, to) = (dirs.attachments(), dir.join(ATTACHMENTS_DIR));
    tauri::async_runtime::spawn_blocking(move || copy_files(&from, &to))
        .await
        .map_err(std::io::Error::other)??;
    setting::set(pool, LAST_ON, &Local::now().date_naive().to_string()).await?;
    if rotate {
        rotate_full(&folder, KEEP_FULL)?;
    }
    describe(&dir).ok_or_else(|| BackupError::NotABackup(dir.display().to_string()).into())
}

/// Whether the daily full backup is due: enabled and not done on `today`.
pub async fn full_due(pool: &SqlitePool, today: NaiveDate) -> AppResult<bool> {
    Ok(
        setting::get(pool, ENABLED).await?.as_deref() != Some("false")
            && setting::get(pool, LAST_ON).await?.as_deref() != Some(today.to_string().as_str()),
    )
}

/// Copies the files of `from` (attachments: no subdirectory) into `to`, created.
fn copy_files(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    let entries = match std::fs::read_dir(from) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Date and time written in a backup name: `complete-20261022-093000-2` → 2026-10-22 09:30:00.
fn stamp_of(name: &str) -> Option<NaiveDateTime> {
    let stamp = name
        .strip_prefix(FULL_PREFIX)
        .or_else(|| name.strip_prefix(IMPORT_PREFIX))?
        .trim_end_matches(".sqlite");
    NaiveDateTime::parse_from_str(stamp.get(..15)?, "%Y%m%d-%H%M%S").ok()
}

/// A backup at `path`: a full backup directory with its data file, or a data file made before
/// an import.
fn describe(path: &Path) -> Option<Backup> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let created_at = stamp_of(&name)?;
    if name.starts_with(FULL_PREFIX) {
        let data = path.join(DATA_FILE_NAME);
        let mut size_bytes = std::fs::metadata(&data).ok()?.len();
        let mut attachment_count = 0;
        if let Ok(entries) = std::fs::read_dir(path.join(ATTACHMENTS_DIR)) {
            for entry in entries.flatten() {
                size_bytes += entry.metadata().map_or(0, |m| m.len());
                attachment_count += 1;
            }
        }
        Some(Backup {
            path: path.display().to_string(),
            kind: BackupKind::Full,
            created_at,
            size_bytes,
            attachment_count,
        })
    } else if name.ends_with(".sqlite") && path.is_file() {
        Some(Backup {
            path: path.display().to_string(),
            kind: BackupKind::Import,
            created_at,
            size_bytes: std::fs::metadata(path).ok()?.len(),
            attachment_count: 0,
        })
    } else {
        None
    }
}

/// Backups of `folder` whose name starts with `prefix`, the most recent first.
fn backups_in(folder: &Path, prefix: &str) -> Vec<Backup> {
    let mut backups: Vec<Backup> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
        .filter_map(|entry| describe(&entry.path()))
        .collect();
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.path.cmp(&a.path)));
    backups
}

/// Full backups and backups made before an import, the most recent first.
pub async fn list(pool: &SqlitePool, dirs: &Dirs) -> AppResult<Vec<Backup>> {
    let mut backups = backups_in(&full_folder(pool, dirs).await?, FULL_PREFIX);
    backups.extend(backups_in(&dirs.default_backups, IMPORT_PREFIX));
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.path.cmp(&a.path)));
    Ok(backups)
}

/// Deletes the oldest full backups of `folder` beyond `keep`.
pub fn rotate_full(folder: &Path, keep: usize) -> std::io::Result<()> {
    for backup in backups_in(folder, FULL_PREFIX).into_iter().skip(keep) {
        std::fs::remove_dir_all(&backup.path)?;
    }
    Ok(())
}

/// Deletes the oldest backups made before an import in `folder` beyond `keep`.
pub fn rotate_import(folder: &Path, keep: usize) -> std::io::Result<()> {
    for backup in backups_in(folder, IMPORT_PREFIX).into_iter().skip(keep) {
        std::fs::remove_file(&backup.path)?;
    }
    Ok(())
}

/// Schedules the restore of a listed backup at the next start, after a full backup of the
/// current data (not rotated, so that the restored backup stays).
pub async fn schedule_restore(pool: &SqlitePool, dirs: &Dirs, path: &str) -> AppResult<()> {
    let listed = list(pool, dirs).await?;
    if !listed.iter().any(|backup| backup.path == path) {
        return Err(BackupError::NotABackup(path.to_owned()).into());
    }
    full(pool, dirs, false).await?;
    std::fs::write(dirs.data.join(RESTORE_MARKER), path)?;
    Ok(())
}

/// Applies a scheduled restore, before the data file is opened: replaces the data file, and
/// the attachments for a full backup. Returns the restored backup. The marker is removed
/// first, so that a failing restore is not tried again at every start.
pub fn apply_pending_restore(data_dir: &Path) -> std::io::Result<Option<PathBuf>> {
    let marker = data_dir.join(RESTORE_MARKER);
    let source = match std::fs::read_to_string(&marker) {
        Ok(source) => PathBuf::from(source),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    std::fs::remove_file(&marker)?;
    let Some(backup) = describe(&source) else {
        return Err(std::io::Error::other(format!(
            "{} is not a backup",
            source.display()
        )));
    };
    let data_source = match backup.kind {
        BackupKind::Full => source.join(DATA_FILE_NAME),
        BackupKind::Import => source.clone(),
    };
    // Copied next to the data file first, then renamed: the data file is never half written.
    let data_file = data_dir.join(DATA_FILE_NAME);
    let staged = data_dir.join(format!("{DATA_FILE_NAME}.restoring"));
    std::fs::copy(&data_source, &staged)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(data_dir.join(format!("{DATA_FILE_NAME}{suffix}")));
    }
    std::fs::rename(&staged, &data_file)?;
    if backup.kind == BackupKind::Full {
        let attachments = data_dir.join(ATTACHMENTS_DIR);
        match std::fs::remove_dir_all(&attachments) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        copy_files(&source.join(ATTACHMENTS_DIR), &attachments)?;
    }
    Ok(Some(source))
}
