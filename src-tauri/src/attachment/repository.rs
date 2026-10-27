use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use super::{Attachment, AttachmentError, MAX_SIZE_BYTES, stored_name};
use crate::error::AppResult;
use crate::operation::{self, timestamp};

/// Attachments of an operation, oldest first.
pub async fn list(pool: &SqlitePool, operation_id: i64) -> AppResult<Vec<Attachment>> {
    let attachments = sqlx::query_as!(
        Attachment,
        r#"SELECT id AS "id!", operation_id, file_name, size_bytes, added_at
           FROM attachment WHERE operation_id = ?1 ORDER BY added_at, id"#,
        operation_id
    )
    .fetch_all(pool)
    .await?;
    Ok(attachments)
}

/// Copies the file at `source` into `dir` and attaches it to an operation.
pub async fn add(
    pool: &SqlitePool,
    dir: &Path,
    operation_id: i64,
    source: &Path,
) -> AppResult<Attachment> {
    let metadata = std::fs::metadata(source)?;
    let file_name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|_| metadata.is_file())
        .ok_or_else(|| AttachmentError::NotAFile(source.display().to_string()))?;
    if metadata.len() > MAX_SIZE_BYTES {
        return Err(AttachmentError::TooLarge {
            max_bytes: MAX_SIZE_BYTES,
        }
        .into());
    }
    operation::get(pool, operation_id).await?;

    let mut tx = pool.begin().await?;
    let size_bytes = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    let now = timestamp();
    // The stored name needs the id: a placeholder unique to this transaction comes first.
    let id = sqlx::query!(
        "INSERT INTO attachment (operation_id, file_name, stored_name, size_bytes, added_at)
         VALUES (?1, ?2, '', ?3, ?4)",
        operation_id,
        file_name,
        size_bytes,
        now
    )
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    let stored = stored_name(id, &file_name);
    sqlx::query!(
        "UPDATE attachment SET stored_name = ?2 WHERE id = ?1",
        id,
        stored
    )
    .execute(&mut *tx)
    .await?;

    std::fs::create_dir_all(dir)?;
    std::fs::copy(source, dir.join(&stored))?;
    // A file copied but not recorded (failed commit) is swept away later.
    tx.commit().await?;

    Ok(Attachment {
        id,
        operation_id,
        file_name,
        size_bytes,
        added_at: now,
    })
}

/// Deletes an attachment and its file.
pub async fn delete(pool: &SqlitePool, dir: &Path, id: i64) -> AppResult<()> {
    let stored = stored_name_of(pool, id).await?;
    sqlx::query!("DELETE FROM attachment WHERE id = ?1", id)
        .execute(pool)
        .await?;
    remove_file(&dir.join(stored))?;
    Ok(())
}

/// Path of the file of an attachment, to open it.
pub async fn path_of(pool: &SqlitePool, dir: &Path, id: i64) -> AppResult<PathBuf> {
    let path = dir.join(stored_name_of(pool, id).await?);
    if !path.is_file() {
        return Err(AttachmentError::FileMissing(id).into());
    }
    Ok(path)
}

/// Removes the files of `dir` that belong to no attachment: those of deleted operations and
/// cancelled imports. Returns their count.
pub async fn sweep(pool: &SqlitePool, dir: &Path) -> AppResult<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let known: HashSet<String> = sqlx::query_scalar!("SELECT stored_name FROM attachment")
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
    let mut removed = 0;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_file() && !known.contains(&name) {
            remove_file(&entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

async fn stored_name_of(pool: &SqlitePool, id: i64) -> AppResult<String> {
    let stored = sqlx::query_scalar!("SELECT stored_name FROM attachment WHERE id = ?1", id)
        .fetch_optional(pool)
        .await?;
    Ok(stored.ok_or(AttachmentError::NotFound(id))?)
}

/// Removes a file; one already gone is fine.
fn remove_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
