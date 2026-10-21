//! Attachments: files put on an operation (bills, receipts), copied into a directory next to
//! the data file. A file goes with its operation: files left without a row are swept away.

mod repository;
#[cfg(test)]
mod tests;

pub use repository::{add, delete, list, path_of, sweep};

use serde::Serialize;
use specta::Type;

use crate::error::ErrorKind;

/// Name of the attachments directory, in the application data directory.
pub const ATTACHMENTS_DIR: &str = "attachments";

/// Largest file accepted, in bytes (25 MB).
pub const MAX_SIZE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: i64,
    pub operation_id: i64,
    /// Original file name.
    pub file_name: String,
    pub size_bytes: i64,
    /// `YYYY-MM-DDTHH:MM:SSZ`.
    pub added_at: String,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AttachmentError {
    #[error("attachment {0} not found")]
    NotFound(i64),

    #[error("{0} is not a file")]
    NotAFile(String),

    #[error("the file is larger than {max_bytes} bytes")]
    TooLarge { max_bytes: u64 },

    #[error("the file of attachment {0} is missing")]
    FileMissing(i64),
}

impl AttachmentError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            AttachmentError::NotFound(_) | AttachmentError::FileMissing(_) => ErrorKind::NotFound,
            _ => ErrorKind::Validation,
        }
    }
}

/// Name of the copy of `file_name` for attachment `id`: `<id>-<name>`, with the characters
/// Windows refuses in file names replaced and the name shortened.
fn stored_name(id: i64, file_name: &str) -> String {
    let safe: String = file_name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    // Keeps the extension when shortening.
    let (stem, extension) = match safe.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && extension.len() <= 10 => {
            (stem, format!(".{extension}"))
        }
        _ => (safe.as_str(), String::new()),
    };
    let stem: String = stem.chars().take(80).collect();
    format!("{id}-{}{extension}", stem.trim_end_matches([' ', '.']))
}
