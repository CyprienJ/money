//! PDF statement parsers, one implementation per bank.
//!
//! PDFium extracts the text of a statement with its positions (`text`); each bank parser
//! recognizes its own statements and turns them into the `RawLine`s shared with the CSV
//! import, along with the statement balances and period when it gives them.

mod fortuneo;
mod french;
#[cfg(test)]
mod tests;
mod text;
mod trade_republic;

pub use text::{DocumentText, extract};

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;

use super::RawLine;
use crate::error::ErrorKind;

/// What a parser reads from a statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub lines: Vec<RawLine>,
    /// Balance before the first line, when the statement gives it.
    pub start_balance_cents: Option<i64>,
    /// Balance after the last line, when the statement gives it.
    pub end_balance_cents: Option<i64>,
    /// First and last days covered by the statement, when it gives them.
    pub period: Option<(NaiveDate, NaiveDate)>,
}

/// A parser for the PDF statements of one bank.
pub trait StatementParser: Send + Sync {
    /// Stable identifier, e.g. `fortuneo`.
    fn id(&self) -> &'static str;
    /// Name shown to the user, e.g. `Fortuneo`.
    fn bank(&self) -> &'static str;
    /// Whether the document is a statement of this bank.
    fn recognizes(&self, document: &DocumentText) -> bool;
    /// Reads a statement recognized by this parser.
    fn parse(&self, document: &DocumentText) -> Result<Statement, PdfError>;
}

/// Parsers of the supported banks, tried in this order.
static PARSERS: &[&dyn StatementParser] = &[&fortuneo::Fortuneo, &trade_republic::TradeRepublic];

/// A bank whose PDF statements can be imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SupportedBank {
    pub id: String,
    pub name: String,
}

impl SupportedBank {
    pub fn of(parser: &dyn StatementParser) -> Self {
        SupportedBank {
            id: parser.id().to_owned(),
            name: parser.bank().to_owned(),
        }
    }
}

pub fn supported_banks() -> Vec<SupportedBank> {
    PARSERS
        .iter()
        .map(|parser| SupportedBank::of(*parser))
        .collect()
}

/// The parser recognizing the document, if any.
pub fn detect(document: &DocumentText) -> Option<&'static dyn StatementParser> {
    detect_among(PARSERS, document)
}

fn detect_among(
    parsers: &[&'static dyn StatementParser],
    document: &DocumentText,
) -> Option<&'static dyn StatementParser> {
    parsers
        .iter()
        .copied()
        .find(|parser| parser.recognizes(document))
}

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("the PDFium library cannot be loaded ({0})")]
    LibraryUnavailable(String),

    #[error("the file is not a readable PDF ({0})")]
    InvalidPdf(String),

    #[error("this PDF is not a statement of a supported bank")]
    UnknownFormat,

    #[error("unexpected statement layout: {0}")]
    Layout(String),
}

impl PdfError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            PdfError::LibraryUnavailable(_) => ErrorKind::Io,
            _ => ErrorKind::Validation,
        }
    }
}
