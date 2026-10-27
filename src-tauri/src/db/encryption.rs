//! Optional encryption of the data file with SQLCipher. The password is the SQLCipher key
//! (derived by SQLCipher itself); nothing allows to recover a forgotten password.
//!
//! The data file is encrypted, re-encrypted with another password or decrypted by exporting it
//! (`sqlcipher_export`) into a file next to it, checked, then renamed over the data file once
//! the pool is closed; the application restarts afterwards. Attachments are not encrypted.

use std::path::{Path, PathBuf};

use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use super::connect;
use crate::error::{AppResult, ErrorKind};

/// Shortest password accepted.
pub const MIN_PASSWORD_LENGTH: usize = 8;

/// First bytes of a SQLite file in clear; an encrypted file starts with random bytes.
const PLAIN_HEADER: &[u8] = b"SQLite format 3\0";

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EncryptionError {
    #[error("wrong password")]
    WrongPassword,

    #[error("the password must have at least {0} characters")]
    PasswordTooShort(u32),

    #[error("the data file is already encrypted")]
    AlreadyEncrypted,

    #[error("the data file is not encrypted")]
    NotEncrypted,
}

impl EncryptionError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::Validation
    }
}

/// Whether the file exists and is encrypted.
pub fn is_encrypted(path: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let mut header = [0; 16];
    match file.read_exact(&mut header) {
        // An empty or tiny file is not a database yet.
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
        Ok(()) => Ok(header != PLAIN_HEADER),
    }
}

/// The password as an SQL string literal.
fn literal(password: &str) -> String {
    format!("'{}'", password.replace('\'', "''"))
}

fn check_password(password: &str) -> Result<(), EncryptionError> {
    if password.chars().count() < MIN_PASSWORD_LENGTH {
        return Err(EncryptionError::PasswordTooShort(
            MIN_PASSWORD_LENGTH as u32,
        ));
    }
    Ok(())
}

/// Opens an encrypted data file and applies pending migrations.
pub async fn open(path: &Path, password: &str) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .pragma("key", literal(password));
    // A wrong key fails on the first read of the file.
    let probe = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await;
    let readable = match probe {
        Ok(pool) => {
            let read = sqlx::query("SELECT count(*) FROM sqlite_master")
                .execute(&pool)
                .await;
            pool.close().await;
            read.is_ok()
        }
        Err(_) => false,
    };
    if !readable {
        return Err(EncryptionError::WrongPassword.into());
    }
    connect(options, SqlitePoolOptions::new()).await
}

/// Exports the database of `pool` into a new file next to `data_file`, encrypted with
/// `password` or in clear for `None`; returns the path of the export, checked.
async fn export(pool: &SqlitePool, data_file: &Path, password: Option<&str>) -> AppResult<PathBuf> {
    let target = data_file.with_extension("sqlite.export");
    // Created empty: `ATTACH` cannot create a file on a connection opened without `create`.
    std::fs::File::create(&target)?;
    let mut conn = pool.acquire().await?;
    // Literals: SQLCipher reads the key of `ATTACH` as written. Both are escaped by `literal`.
    let attach = format!(
        "ATTACH DATABASE {} AS export KEY {}",
        literal(&target.to_string_lossy()),
        literal(password.unwrap_or(""))
    );
    sqlx::query(sqlx::AssertSqlSafe(attach))
        .execute(&mut *conn)
        .await?;
    let exported = sqlx::query("SELECT sqlcipher_export('export')")
        .execute(&mut *conn)
        .await;
    // `user_version` is not exported.
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *conn)
        .await?;
    if exported.is_ok() {
        // An integer: nothing to escape.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA export.user_version = {version}"
        )))
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query("DETACH DATABASE export")
        .execute(&mut *conn)
        .await?;
    exported?;
    drop(conn);

    // The export opens with its key and has the same tables.
    let check = match password {
        Some(password) => open(&target, password).await,
        None => super::open(&target).await,
    };
    match check {
        Ok(copy) => copy.close().await,
        Err(error) => {
            let _ = std::fs::remove_file(&target);
            return Err(error);
        }
    }
    Ok(target)
}

/// Replaces the data file with an export, once `pool` is closed.
async fn replace(pool: &SqlitePool, data_file: &Path, export: &Path) -> AppResult<()> {
    pool.close().await;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = data_file.as_os_str().to_owned();
        name.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(name));
    }
    std::fs::rename(export, data_file)?;
    Ok(())
}

/// Encrypts the data file in clear with `password`. Closes `pool`: restart afterwards.
pub async fn encrypt(pool: &SqlitePool, data_file: &Path, password: &str) -> AppResult<()> {
    check_password(password)?;
    if is_encrypted(data_file)? {
        return Err(EncryptionError::AlreadyEncrypted.into());
    }
    let export = export(pool, data_file, Some(password)).await?;
    replace(pool, data_file, &export).await
}

/// Encrypts the data file with a new password. Closes `pool`: restart afterwards.
pub async fn change_password(
    pool: &SqlitePool,
    data_file: &Path,
    current: &str,
    new: &str,
) -> AppResult<()> {
    check_password(new)?;
    verify(data_file, current).await?;
    let export = export(pool, data_file, Some(new)).await?;
    replace(pool, data_file, &export).await
}

/// Decrypts the data file. Closes `pool`: restart afterwards.
pub async fn decrypt(pool: &SqlitePool, data_file: &Path, current: &str) -> AppResult<()> {
    verify(data_file, current).await?;
    let export = export(pool, data_file, None).await?;
    replace(pool, data_file, &export).await
}

/// Checks the password of the encrypted data file.
async fn verify(data_file: &Path, password: &str) -> AppResult<()> {
    if !is_encrypted(data_file)? {
        return Err(EncryptionError::NotEncrypted.into());
    }
    open(data_file, password).await?.close().await;
    Ok(())
}

#[cfg(test)]
mod tests;
