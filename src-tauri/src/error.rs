use serde::{Serialize, Serializer};
use specta::Type;
use specta::datatype::DataType;

use crate::account::AccountError;
use crate::importing::batch::ImportError;
use crate::importing::csv::{CsvError, CsvMappingError};
use crate::importing::pdf::PdfError;
use crate::labeling::rules::RuleError;
use crate::operation::OperationError;
use crate::operation::category::CategoryError;
use crate::operation::schedule::ScheduleError;
use crate::payee::PayeeError;
use crate::stats::StatsError;
use crate::tag::TagError;

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
    Schedule(#[from] ScheduleError),

    #[error(transparent)]
    Rule(#[from] RuleError),

    #[error(transparent)]
    Payee(#[from] PayeeError),

    #[error(transparent)]
    Tag(#[from] TagError),

    #[error(transparent)]
    Stats(#[from] StatsError),

    #[error(transparent)]
    Csv(#[from] CsvError),

    #[error(transparent)]
    CsvMapping(#[from] CsvMappingError),

    #[error(transparent)]
    Import(#[from] ImportError),

    #[error(transparent)]
    Pdf(#[from] PdfError),
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

/// Business error with its code and parameters, for a message in French on the frontend.
#[derive(Serialize, Type)]
#[serde(tag = "domain", content = "error", rename_all = "camelCase")]
enum ErrorDetail {
    Account(AccountError),
    Operation(OperationError),
    Category(CategoryError),
    Schedule(ScheduleError),
    Rule(RuleError),
    Payee(PayeeError),
    Tag(TagError),
    Stats(StatsError),
    Csv(CsvError),
    CsvMapping(CsvMappingError),
    Import(ImportError),
    Pdf(PdfError),
}

/// Error returned by commands to the frontend: `{ kind, message, detail }`. `message` is the
/// English description; `detail` is `null` for database, migration and I/O errors.
#[derive(Serialize, Type)]
#[serde(rename = "AppError")]
struct ErrorPayload {
    kind: ErrorKind,
    message: String,
    detail: Option<ErrorDetail>,
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
            AppError::Schedule(error) => error.kind(),
            AppError::Rule(error) => error.kind(),
            AppError::Payee(error) => error.kind(),
            AppError::Tag(error) => error.kind(),
            AppError::Stats(error) => error.kind(),
            AppError::Csv(error) => error.kind(),
            AppError::CsvMapping(error) => error.kind(),
            AppError::Import(error) => error.kind(),
            AppError::Pdf(error) => error.kind(),
        }
    }
}

impl AppError {
    fn detail(&self) -> Option<ErrorDetail> {
        Some(match self {
            AppError::Database(_) | AppError::Migration(_) | AppError::Io(_) => return None,
            AppError::Account(error) => ErrorDetail::Account(error.clone()),
            AppError::Operation(error) => ErrorDetail::Operation(error.clone()),
            AppError::Category(error) => ErrorDetail::Category(error.clone()),
            AppError::Schedule(error) => ErrorDetail::Schedule(error.clone()),
            AppError::Rule(error) => ErrorDetail::Rule(error.clone()),
            AppError::Payee(error) => ErrorDetail::Payee(error.clone()),
            AppError::Tag(error) => ErrorDetail::Tag(error.clone()),
            AppError::Stats(error) => ErrorDetail::Stats(error.clone()),
            AppError::Csv(error) => ErrorDetail::Csv(error.clone()),
            AppError::CsvMapping(error) => ErrorDetail::CsvMapping(error.clone()),
            AppError::Import(error) => ErrorDetail::Import(error.clone()),
            AppError::Pdf(error) => ErrorDetail::Pdf(error.clone()),
        })
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ErrorPayload {
            kind: self.kind(),
            message: self.to_string(),
            detail: self.detail(),
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
        assert!(json["detail"].is_null());
    }

    #[test]
    fn serializes_business_errors_with_code_and_params() {
        let json = serde_json::to_value(AppError::from(CsvError::InvalidDate {
            line: 4,
            value: "32/09/2026".to_owned(),
        }))
        .unwrap();
        assert_eq!(
            json["detail"],
            serde_json::json!({
                "domain": "csv",
                "error": { "code": "invalidDate", "params": { "line": 4, "value": "32/09/2026" } }
            })
        );
        let json = serde_json::to_value(AppError::from(AccountError::EmptyName)).unwrap();
        assert_eq!(
            json["detail"],
            serde_json::json!({ "domain": "account", "error": { "code": "emptyName" } })
        );
        let json = serde_json::to_value(AppError::from(AccountError::NotFound(3))).unwrap();
        assert_eq!(json["detail"]["error"]["params"], 3);
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
