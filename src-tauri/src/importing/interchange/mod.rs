//! Interchange formats of personal finance software and banks: OFX (Microsoft Money, bank
//! downloads) and QIF (Microsoft Money, Quicken). Both are read by hand, tolerantly: one bank
//! or card account per file, investment statements refused.

pub mod ofx;
pub mod qif;
#[cfg(test)]
mod tests;

use encoding_rs::WINDOWS_1252;
use serde::Serialize;
use specta::Type;

use crate::error::ErrorKind;

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InterchangeError {
    #[error("the file is not an OFX file")]
    NotOfx,

    #[error("the file is not a QIF file")]
    NotQif,

    #[error("the file holds no bank or card account")]
    NoAccount,

    #[error("the file holds {0} accounts: export one account per file")]
    SeveralAccounts(u32),

    #[error("investment statements cannot be imported")]
    InvestmentStatement,

    #[error("line {line}: invalid date {text:?}")]
    InvalidDate { line: u32, text: String },

    #[error("line {line}: invalid amount {text:?}")]
    InvalidAmount { line: u32, text: String },

    #[error("line {line}: a transaction without a date or an amount")]
    IncompleteTransaction { line: u32 },

    #[error("the file is in {file}, the account in {account}")]
    CurrencyMismatch { file: String, account: String },
}

impl InterchangeError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::Validation
    }
}

/// Text of a file in UTF-8 (byte order mark removed), or else in Windows-1252, the encoding of
/// the French versions of Money.
fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

/// Cents of the amount `units.decimals`, rounded half up, without going through floating
/// point; `None` unless both parts are digits.
fn cents(negative: bool, units: &str, decimals: &str) -> Option<i64> {
    if units.is_empty() && decimals.is_empty()
        || !units.bytes().all(|b| b.is_ascii_digit())
        || !decimals.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let digit = |index: usize| -> i64 {
        decimals
            .as_bytes()
            .get(index)
            .map_or(0, |b| i64::from(b - b'0'))
    };
    let units: i64 = if units.is_empty() {
        0
    } else {
        units.parse().ok()?
    };
    let cents = units
        .checked_mul(100)?
        .checked_add(digit(0) * 10 + digit(1) + i64::from(digit(2) >= 5))?;
    Some(if negative { -cents } else { cents })
}

/// Sign and the rest of an amount: "-", "−" or "+" in front, spaces removed.
fn split_sign(text: &str) -> (bool, String) {
    let compact: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '\u{202f}')
        .collect();
    if let Some(rest) = compact
        .strip_prefix('-')
        .or_else(|| compact.strip_prefix('−'))
    {
        (true, rest.to_owned())
    } else {
        (
            false,
            compact.strip_prefix('+').unwrap_or(&compact).to_owned(),
        )
    }
}

/// An amount where the decimal separator, "." or ",", is the only separator (OFX).
fn parse_plain_amount(text: &str) -> Option<i64> {
    let (negative, rest) = split_sign(text);
    let (units, decimals) = rest.split_once(['.', ',']).unwrap_or((&rest, ""));
    cents(negative, units, decimals)
}

/// An amount with thousands separators maybe (QIF): "1,234.56", "1 234,56", "1.234,56",
/// "-12,5". The last "." or "," followed by one or two digits is the decimal separator; any
/// other one separates thousands.
fn parse_grouped_amount(text: &str) -> Option<i64> {
    let (negative, rest) = split_sign(text);
    let (units, decimals) = match rest.rfind(['.', ',']) {
        Some(index) if (2..=3).contains(&(rest.len() - index)) => {
            (&rest[..index], &rest[index + 1..])
        }
        _ => (rest.as_str(), ""),
    };
    let units: String = units
        .chars()
        .filter(|c| !matches!(c, '.' | ',' | '\''))
        .collect();
    cents(negative, &units, decimals)
}

/// Number of the line of a byte offset, from 1.
fn line_at(text: &str, offset: usize) -> u32 {
    let count = text[..offset].bytes().filter(|b| *b == b'\n').count();
    u32::try_from(count + 1).unwrap_or(u32::MAX)
}
