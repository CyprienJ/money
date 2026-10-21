//! Payees: each normalized label belongs to exactly one payee, created automatically with the
//! readable form of the label as its name. The user can rename payees, merge several into one
//! and detach a label from a merged payee.

mod repository;
#[cfg(test)]
mod tests;

pub use repository::{assign_missing, detach, list, merge, rename};
pub(crate) use repository::{follow_label, names_of, resolve};

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;

use crate::error::ErrorKind;

/// A payee with its labels and the activity of its operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Payee {
    pub id: i64,
    pub name: String,
    /// Normalized labels merged into this payee, alphabetically.
    pub labels: Vec<String>,
    pub operation_count: i64,
    /// Sum of the operation amounts, internal transfers excluded; `None` when all its
    /// operations are internal transfers.
    pub total_cents: Option<i64>,
    pub last_date: Option<NaiveDate>,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PayeeError {
    #[error("payee {0} not found")]
    NotFound(i64),

    #[error("label {0} belongs to no payee")]
    LabelNotFound(String),

    #[error("payee name cannot be empty")]
    EmptyName,

    #[error("another payee is already named {name}")]
    NameTaken { name: String, payee_id: i64 },

    #[error("a payee cannot be merged into itself")]
    MergeIntoItself,

    #[error("the only label of a payee cannot be detached")]
    LastLabel,
}

impl PayeeError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            PayeeError::NotFound(_) | PayeeError::LabelNotFound(_) => ErrorKind::NotFound,
            _ => ErrorKind::Validation,
        }
    }
}

/// Small French words kept in lowercase inside a default payee name.
const SMALL_WORDS: [&str; 16] = [
    "à", "au", "aux", "d", "de", "des", "du", "en", "et", "l", "la", "le", "les", "par", "pour",
    "sur",
];

/// Default name of a payee, the readable form of its normalized label: a capital at the start
/// of each word, small words in lowercase, words with digits unchanged. `PHARMACIE DU CENTRE`
/// → `Pharmacie du Centre`. Same as `displayLabel` on the frontend.
pub fn default_name(normalized_label: &str) -> String {
    let name = normalized_label
        .split(' ')
        .enumerate()
        .map(|(index, word)| {
            if word.chars().any(|c| c.is_ascii_digit()) {
                return word.to_owned();
            }
            let lower = word.to_lowercase();
            if index > 0 && SMALL_WORDS.contains(&lower.as_str()) {
                return lower;
            }
            // Capitalize each part of "SAINT-MARTIN" or "L'ARBRE".
            let mut capitalize = true;
            lower
                .chars()
                .flat_map(|c| {
                    let upper = capitalize && c.is_alphabetic();
                    capitalize = matches!(c, '-' | '\'' | '’');
                    let mapped: Vec<char> = if upper {
                        c.to_uppercase().collect()
                    } else {
                        vec![c]
                    };
                    mapped
                })
                .collect()
        })
        .collect::<Vec<String>>()
        .join(" ");
    if name.trim().is_empty() {
        "Sans libellé".to_owned()
    } else {
        name
    }
}
