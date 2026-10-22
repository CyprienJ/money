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

use serde::Serialize;
use specta::Type;

pub use super::Statement;
use crate::error::ErrorKind;
use crate::investment::TransactionKind;

/// A buy or a sell of securities read from a statement. The amount is the total paid or
/// received, fees included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub date: chrono::NaiveDate,
    pub kind: TransactionKind,
    pub isin: String,
    /// Name of the security as written by the bank, e.g. "ASML HOLDING EO -,09".
    pub name: String,
    /// Millionths of a unit.
    pub quantity_micro: i64,
    /// Positive.
    pub amount_cents: i64,
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
    /// Reads the securities trades of a statement recognized by this parser; `None` when the
    /// statements of this bank give none.
    fn parse_trades(&self, _document: &DocumentText) -> Result<Option<Vec<Trade>>, PdfError> {
        Ok(None)
    }
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

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PdfError {
    #[error("the PDFium library cannot be loaded ({0})")]
    LibraryUnavailable(String),

    #[error("the file is not a readable PDF ({0})")]
    InvalidPdf(String),

    #[error("this PDF is not a statement of a supported bank")]
    UnknownFormat,

    #[error("unexpected statement layout: {0}")]
    Layout(String),

    #[error("the statements of {0} give no securities transactions")]
    NoTrades(String),
}

impl PdfError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            PdfError::LibraryUnavailable(_) => ErrorKind::Io,
            _ => ErrorKind::Validation,
        }
    }
}
