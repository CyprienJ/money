//! Import batches: backup, transactional insertion, cancellation.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use specta::Type;
use sqlx::SqlitePool;

use super::RawLine;
use crate::error::{AppResult, ErrorKind};
use crate::labeling::{memory, normalize_label};
use crate::operation::{check_account, internal_transfer_flag, timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceType {
    Csv,
    Pdf,
}

/// Result of `start balance + Σ operations = end balance` on the statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BalanceCheckStatus {
    Ok,
    Mismatch,
    /// The statement gives no balances (CSV files).
    NotAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatch {
    pub id: i64,
    pub account_id: i64,
    pub source_type: SourceType,
    pub source_file_name: String,
    /// SHA-256 of the file, in hexadecimal.
    pub source_file_hash: String,
    pub imported_at: DateTime<Utc>,
    pub statement_start_balance_cents: Option<i64>,
    pub statement_end_balance_cents: Option<i64>,
    pub balance_check_status: BalanceCheckStatus,
    pub operation_count: i64,
}

/// Source and statement balances of an import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImportBatch {
    pub account_id: i64,
    pub source_type: SourceType,
    pub source_file_name: String,
    pub source_file_hash: String,
    pub statement_start_balance_cents: Option<i64>,
    pub statement_end_balance_cents: Option<i64>,
    pub balance_check_status: BalanceCheckStatus,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ImportError {
    #[error("import {0} not found")]
    NotFound(i64),

    #[error("nothing to import")]
    NoLine,

    #[error("statement line with an empty label or a zero amount")]
    InvalidLine,

    #[error("the file changed since its preview, start the import again")]
    FileChanged,

    #[error("line {0} of the preview does not exist")]
    LineOutOfRange(u32),

    #[error("line {0} is already in the account and cannot be imported again")]
    DuplicateLine(u32),
}

impl ImportError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            ImportError::NotFound(_) => ErrorKind::NotFound,
            _ => ErrorKind::Validation,
        }
    }
}

/// SHA-256 of a statement file, in hexadecimal, to detect a file imported twice.
pub fn file_hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Copies the data file to `dir/money-YYYYMMDD-HHMMSS.sqlite` (local time) and returns the
/// copy path. `VACUUM INTO` writes a consistent copy while the pool stays open.
pub async fn backup(pool: &SqlitePool, dir: &Path) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let mut path = dir.join(format!("money-{stamp}.sqlite"));
    let mut suffix = 1;
    while path.exists() {
        suffix += 1;
        path = dir.join(format!("money-{stamp}-{suffix}.sqlite"));
    }
    sqlx::query("VACUUM INTO ?1")
        .bind(path.to_string_lossy().into_owned())
        .execute(pool)
        .await?;
    Ok(path)
}

/// A statement line to import, with the category it receives (`None`: to categorize).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportLine {
    pub line: RawLine,
    pub category_id: Option<i64>,
    /// The user chose this category: remember it for the label (category memory).
    pub chosen: bool,
}

/// Number of backups made by [`backup`] in `dir`; 0 when the directory does not exist yet.
pub fn backup_count(dir: &Path) -> AppResult<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut count = 0;
    for entry in entries {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("money-") && name.ends_with(".sqlite") {
            count += 1;
        }
    }
    Ok(count)
}

/// Backs up the data file into `backup_dir`, then inserts the batch and one operation per
/// line, with its category, in a single transaction. Duplicates and lines dated before the
/// account opening must have been excluded beforehand: such a line fails the whole import.
pub async fn import(
    pool: &SqlitePool,
    backup_dir: &Path,
    batch: NewImportBatch,
    lines: &[ImportLine],
) -> AppResult<ImportBatch> {
    let Some(first_date) = lines.iter().map(|l| l.line.date).min() else {
        return Err(ImportError::NoLine.into());
    };
    if lines
        .iter()
        .any(|l| l.line.label.trim().is_empty() || l.line.amount_cents == 0)
    {
        return Err(ImportError::InvalidLine.into());
    }
    // Internal transfer flag of each category, which also checks that it exists.
    let mut internal_transfer = HashMap::new();
    {
        let mut conn = pool.acquire().await?;
        check_account(&mut conn, batch.account_id, first_date).await?;
        for category_id in lines.iter().map(|l| l.category_id) {
            if let Entry::Vacant(entry) = internal_transfer.entry(category_id) {
                entry.insert(internal_transfer_flag(&mut conn, category_id).await?);
            }
        }
    }

    backup(pool, backup_dir).await?;

    let now = timestamp();
    let mut tx = pool.begin().await?;
    let batch_id = sqlx::query!(
        "INSERT INTO import_batch
         (account_id, source_type, source_file_name, source_file_hash, imported_at,
          statement_start_balance_cents, statement_end_balance_cents, balance_check_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        batch.account_id,
        batch.source_type,
        batch.source_file_name,
        batch.source_file_hash,
        now,
        batch.statement_start_balance_cents,
        batch.statement_end_balance_cents,
        batch.balance_check_status
    )
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    for ImportLine {
        line,
        category_id,
        chosen,
    } in lines
    {
        let label = line.label.trim();
        let normalized_label = normalize_label(label);
        let is_internal_transfer = internal_transfer[category_id];
        let operation_id = sqlx::query!(
            "INSERT INTO operation
             (account_id, date, value_date, amount_cents, raw_label, normalized_label,
              import_batch_id, is_internal_transfer, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            batch.account_id,
            line.date,
            line.value_date,
            line.amount_cents,
            label,
            normalized_label,
            batch_id,
            is_internal_transfer,
            now
        )
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();
        sqlx::query!(
            "INSERT INTO operation_split (operation_id, category_id, amount_cents)
             VALUES (?1, ?2, ?3)",
            operation_id,
            category_id,
            line.amount_cents
        )
        .execute(&mut *tx)
        .await?;
        if *chosen {
            memory::remember(&mut tx, &normalized_label, *category_id, &now).await?;
        }
    }
    tx.commit().await?;
    get(pool, batch_id).await
}

/// Cancels an import: deletes the batch, its operations and their splits.
pub async fn cancel(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let deleted = sqlx::query!("DELETE FROM import_batch WHERE id = ?1", id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(ImportError::NotFound(id).into());
    }
    Ok(())
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<ImportBatch> {
    let batch = sqlx::query_as!(
        ImportBatch,
        r#"SELECT b.id AS "id!", b.account_id, b.source_type AS "source_type: SourceType",
                  b.source_file_name, b.source_file_hash,
                  b.imported_at AS "imported_at: DateTime<Utc>",
                  b.statement_start_balance_cents, b.statement_end_balance_cents,
                  b.balance_check_status AS "balance_check_status: BalanceCheckStatus",
                  (SELECT COUNT(*) FROM operation o WHERE o.import_batch_id = b.id)
                      AS "operation_count!: i64"
           FROM import_batch b
           WHERE b.id = ?1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(batch.ok_or(ImportError::NotFound(id))?)
}

/// Imports of an account, most recent first.
pub async fn list(pool: &SqlitePool, account_id: i64) -> AppResult<Vec<ImportBatch>> {
    let batches = sqlx::query_as!(
        ImportBatch,
        r#"SELECT b.id AS "id!", b.account_id, b.source_type AS "source_type: SourceType",
                  b.source_file_name, b.source_file_hash,
                  b.imported_at AS "imported_at: DateTime<Utc>",
                  b.statement_start_balance_cents, b.statement_end_balance_cents,
                  b.balance_check_status AS "balance_check_status: BalanceCheckStatus",
                  (SELECT COUNT(*) FROM operation o WHERE o.import_batch_id = b.id)
                      AS "operation_count!: i64"
           FROM import_batch b
           WHERE b.account_id = ?1
           ORDER BY b.imported_at DESC, b.id DESC"#,
        account_id
    )
    .fetch_all(pool)
    .await?;
    Ok(batches)
}

/// Imports of the same file, in any account, most recent first.
pub async fn find_by_hash(pool: &SqlitePool, hash: &str) -> AppResult<Vec<ImportBatch>> {
    let batches = sqlx::query_as!(
        ImportBatch,
        r#"SELECT b.id AS "id!", b.account_id, b.source_type AS "source_type: SourceType",
                  b.source_file_name, b.source_file_hash,
                  b.imported_at AS "imported_at: DateTime<Utc>",
                  b.statement_start_balance_cents, b.statement_end_balance_cents,
                  b.balance_check_status AS "balance_check_status: BalanceCheckStatus",
                  (SELECT COUNT(*) FROM operation o WHERE o.import_batch_id = b.id)
                      AS "operation_count!: i64"
           FROM import_batch b
           WHERE b.source_file_hash = ?1
           ORDER BY b.imported_at DESC, b.id DESC"#,
        hash
    )
    .fetch_all(pool)
    .await?;
    Ok(batches)
}

#[cfg(test)]
mod tests;
