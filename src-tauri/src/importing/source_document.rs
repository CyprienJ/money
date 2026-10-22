//! Source documents: every imported file is kept in a directory next to the data file, under
//! the SHA-256 of its content (`<hash>.<extension>`): a file imported into two accounts is
//! kept once. An operation, or a trade, opens the statement it comes from: a PDF with the
//! application of the system, a text file (CSV, OFX, QIF) in the application. A copy that no
//! import refers to anymore (cancelled imports) is swept away.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::interchange::decode;
use crate::error::{AppResult, ErrorKind};

/// Name of the directory of the kept statements, in the application data directory.
pub const STATEMENTS_DIR: &str = "statements";

/// How a document is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DocumentKind {
    /// Opened with the application of the system.
    Pdf,
    /// Shown in the application, line by line.
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceDocument {
    pub import_batch_id: i64,
    /// Original file name.
    pub file_name: String,
    pub imported_at: DateTime<Utc>,
    pub kind: DocumentKind,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SourceDocumentError {
    #[error("the file of import {0} was not kept")]
    NotKept(i64),

    #[error("the file of import {0} is missing")]
    FileMissing(i64),

    #[error("the file of import {0} is not a text file")]
    NotText(i64),
}

impl SourceDocumentError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::NotFound
    }
}

/// Name of the copy of a file: its hash and its extension, in lowercase.
pub fn stored_name(file_hash: &str, file_name: &str) -> String {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| !e.is_empty() && e.len() <= 8 && e.bytes().all(|b| b.is_ascii_alphanumeric()));
    match extension {
        Some(extension) => format!("{file_hash}.{extension}"),
        None => file_hash.to_owned(),
    }
}

/// Copies an imported file into `dir`, unless a copy of the same content is there already.
/// Returns the name of the copy.
pub fn store(dir: &Path, file_hash: &str, file_name: &str, bytes: &[u8]) -> AppResult<String> {
    let name = stored_name(file_hash, file_name);
    let path = dir.join(&name);
    if !path.is_file() {
        std::fs::create_dir_all(dir)?;
        // Written aside then renamed: a copy is never half written.
        let partial = dir.join(format!("{name}.partial"));
        std::fs::write(&partial, bytes)?;
        std::fs::rename(&partial, &path)?;
    }
    Ok(name)
}

/// Removes the files of `dir` that no import refers to. Returns their count.
pub async fn sweep(pool: &SqlitePool, dir: &Path) -> AppResult<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let kept: HashSet<String> = sqlx::query_scalar!(
        r#"SELECT DISTINCT stored_file AS "stored_file!" FROM import_batch
           WHERE stored_file IS NOT NULL"#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();
    let mut removed = 0;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_file() && !kept.contains(&name) {
            std::fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// The kept file of an import.
pub async fn get(pool: &SqlitePool, import_batch_id: i64) -> AppResult<SourceDocument> {
    let (document, _) = find(pool, import_batch_id).await?;
    Ok(document)
}

/// Path of the kept file of an import, which must exist.
pub async fn path_of(pool: &SqlitePool, dir: &Path, import_batch_id: i64) -> AppResult<PathBuf> {
    let (_, stored_file) = find(pool, import_batch_id).await?;
    let path = dir.join(stored_file);
    if !path.is_file() {
        return Err(SourceDocumentError::FileMissing(import_batch_id).into());
    }
    Ok(path)
}

/// Text of a kept CSV, OFX or QIF file, in UTF-8 or else Windows-1252.
pub async fn text_of(pool: &SqlitePool, dir: &Path, import_batch_id: i64) -> AppResult<String> {
    if get(pool, import_batch_id).await?.kind != DocumentKind::Text {
        return Err(SourceDocumentError::NotText(import_batch_id).into());
    }
    let bytes = std::fs::read(path_of(pool, dir, import_batch_id).await?)?;
    Ok(decode(&bytes))
}

async fn find(pool: &SqlitePool, import_batch_id: i64) -> AppResult<(SourceDocument, String)> {
    let row = sqlx::query!(
        r#"SELECT id AS "id!", source_file_name, imported_at AS "imported_at: DateTime<Utc>",
                  stored_file
           FROM import_batch
           WHERE id = ?1"#,
        import_batch_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(super::batch::ImportError::NotFound(import_batch_id))?;
    let stored_file = row
        .stored_file
        .ok_or(SourceDocumentError::NotKept(import_batch_id))?;
    let kind = if stored_file.ends_with(".pdf") {
        DocumentKind::Pdf
    } else {
        DocumentKind::Text
    };
    let document = SourceDocument {
        import_batch_id: row.id,
        file_name: row.source_file_name,
        imported_at: row.imported_at,
        kind,
    };
    Ok((document, stored_file))
}

#[cfg(test)]
mod tests;
