//! Tags: free labels put on operations. A tag is created the first time it is put on an
//! operation; the user can rename and delete tags.

mod repository;
#[cfg(test)]
mod tests;

pub use repository::{delete, list, rename};
pub(crate) use repository::{names_by_operation, names_of, set_for_operation};

use serde::Serialize;
use specta::Type;

use crate::error::ErrorKind;

/// A tag with the count of its operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub operation_count: i64,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TagError {
    #[error("tag {0} not found")]
    NotFound(i64),

    #[error("tag name cannot be empty")]
    EmptyName,

    #[error("another tag is already named {0}")]
    NameTaken(String),
}

impl TagError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            TagError::NotFound(_) => ErrorKind::NotFound,
            _ => ErrorKind::Validation,
        }
    }
}

/// Tag name without surrounding nor repeated spaces; `None` when nothing is left.
pub fn clean_name(name: &str) -> Option<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty()).then_some(name)
}
