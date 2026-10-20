//! CSV statement parser and column mappings.

mod amount;
mod mapping;
mod parser;
#[cfg(test)]
mod tests;

pub(crate) use amount::parse_cents;
pub use mapping::{
    CsvMappingError, MappingSuggestion, NewCsvMapping, SavedCsvMapping, create, delete, get, list,
    suggest, update,
};
pub use parser::{head, parse};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::ErrorKind;

/// Text encoding of a CSV file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum CsvEncoding {
    #[serde(rename = "UTF-8")]
    Utf8,
    #[serde(rename = "ISO-8859-1")]
    Iso8859_1,
    #[serde(rename = "WINDOWS-1252")]
    Windows1252,
}

impl CsvEncoding {
    fn encoding(self) -> &'static encoding_rs::Encoding {
        match self {
            CsvEncoding::Utf8 => encoding_rs::UTF_8,
            // Bank exports labelled ISO-8859-1 are decoded as its Windows-1252 superset,
            // as browsers do.
            CsvEncoding::Iso8859_1 | CsvEncoding::Windows1252 => encoding_rs::WINDOWS_1252,
        }
    }
}

/// Decimal separator of the amounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum DecimalSeparator {
    #[serde(rename = ",")]
    Comma,
    #[serde(rename = ".")]
    Dot,
}

impl DecimalSeparator {
    fn decimal(self) -> char {
        match self {
            DecimalSeparator::Comma => ',',
            DecimalSeparator::Dot => '.',
        }
    }

    /// The other character, accepted as a thousands separator.
    fn thousands(self) -> char {
        match self {
            DecimalSeparator::Comma => '.',
            DecimalSeparator::Dot => ',',
        }
    }
}

/// Where the amount of a line is read. Column indexes are zero-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AmountColumns {
    /// One signed amount column.
    Signed { amount: u32 },
    /// Separate debit and credit columns; their sign is ignored.
    DebitCredit { debit: u32, credit: u32 },
}

/// How to read the CSV export of a bank. Column indexes are zero-based.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CsvMapping {
    pub delimiter: char,
    pub encoding: CsvEncoding,
    /// `chrono` format, e.g. `%d/%m/%Y`.
    pub date_format: String,
    pub decimal_separator: DecimalSeparator,
    pub column_date: u32,
    pub column_label: u32,
    pub amount_columns: AmountColumns,
    /// Lines ignored at the top of the file, column headers included.
    pub skip_lines: u32,
}

impl CsvMapping {
    /// Checks the settings that do not depend on the file; returns the delimiter byte.
    pub fn validate(&self) -> Result<u8, CsvError> {
        let delimiter = u8::try_from(self.delimiter)
            .ok()
            .filter(u8::is_ascii)
            .ok_or(CsvError::InvalidDelimiter(self.delimiter))?;
        if self.date_format.trim().is_empty() {
            return Err(CsvError::EmptyDateFormat);
        }
        let invalid = chrono::format::StrftimeItems::new(&self.date_format)
            .any(|item| matches!(item, chrono::format::Item::Error));
        if invalid {
            return Err(CsvError::InvalidDateFormat(self.date_format.clone()));
        }
        Ok(delimiter)
    }
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CsvError {
    #[error("the CSV delimiter must be a single ASCII character, got {0:?}")]
    InvalidDelimiter(char),

    #[error("the date format cannot be empty")]
    EmptyDateFormat,

    #[error("invalid date format {0:?}")]
    InvalidDateFormat(String),

    #[error("the file is not valid {0}")]
    InvalidEncoding(&'static str),

    #[error("line {line}: malformed CSV ({message})")]
    Malformed { line: u64, message: String },

    #[error("line {line}: column {column} is missing")]
    MissingColumn { line: u64, column: u32 },

    #[error("line {line}: invalid date {value:?}")]
    InvalidDate { line: u64, value: String },

    #[error("line {line}: invalid amount {value:?}")]
    InvalidAmount { line: u64, value: String },

    #[error("line {line}: no amount")]
    MissingAmount { line: u64 },

    #[error("line {line}: zero amount")]
    ZeroAmount { line: u64 },

    #[error("line {line}: both a debit and a credit")]
    BothDebitAndCredit { line: u64 },

    #[error("line {line}: empty label")]
    EmptyLabel { line: u64 },
}

impl CsvError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::Validation
    }
}
