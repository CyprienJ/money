use serde::{Serialize, Serializer};
use specta::Type;
use specta::datatype::DataType;

use crate::account::AccountError;
use crate::importing::batch::ImportError;
use crate::importing::csv::{CsvError, CsvMappingError};
use crate::operation::OperationError;
use crate::operation::category::CategoryError;

/// Error type returned by the domain layer and propagated to the frontend.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Account(#[from] AccountError),

    #[error(transparent)]
    Operation(#[from] OperationError),

    #[error(transparent)]
    Category(#[from] CategoryError),

    #[error(transparent)]
    Csv(#[from] CsvError),

    #[error(transparent)]
    CsvMapping(#[from] CsvMappingError),

    #[error(transparent)]
    Import(#[from] ImportError),
}

/// Stable identifier the frontend can match on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Database,
    Migration,
    Io,
    /// The requested entity does not exist.
    NotFound,
    /// The input breaks a business rule; the message says which.
    Validation,
}

/// Error returned by commands to the frontend: `{ kind, message }`.
#[derive(Serialize, Type)]
#[serde(rename = "AppError")]
struct ErrorPayload {
    kind: ErrorKind,
    message: String,
}

impl AppError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            AppError::Database(_) => ErrorKind::Database,
            AppError::Migration(_) => ErrorKind::Migration,
            AppError::Io(_) => ErrorKind::Io,
            AppError::Account(error) => error.kind(),
            AppError::Operation(error) => error.kind(),
            AppError::Category(error) => error.kind(),
            AppError::Csv(error) => error.kind(),
            AppError::CsvMapping(error) => error.kind(),
            AppError::Import(error) => error.kind(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ErrorPayload {
            kind: self.kind(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

impl Type for AppError {
    fn definition(types: &mut specta::Types) -> DataType {
        ErrorPayload::definition(types)
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_message() {
        let error = AppError::Io(std::io::Error::other("disk full"));
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["kind"], "io");
        assert_eq!(json["message"], "I/O error: disk full");
    }

    #[test]
    fn maps_domain_errors_to_generic_kinds() {
        assert_eq!(
            AppError::from(AccountError::NotFound(3)).kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            AppError::from(AccountError::EmptyName).kind(),
            ErrorKind::Validation
        );
        assert_eq!(
            AppError::from(AccountError::NotFound(3)).to_string(),
            "account 3 not found"
        );
    }
}
