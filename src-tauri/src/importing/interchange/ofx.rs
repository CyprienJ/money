//! OFX statements: OFX 1.x (SGML, leaf elements without closing tags) and 2.x (XML), read as
//! a flat list of tags. Bank (`STMTRS`) and credit card (`CCSTMTRS`) statements only.

use chrono::NaiveDate;

use super::{InterchangeError, decode, line_at, parse_plain_amount};
use crate::importing::{RawLine, Statement};

/// A statement read from an OFX file, with its currency (`CURDEF`) when given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfxStatement {
    pub statement: Statement,
    pub currency: Option<String>,
}

/// An opening or closing tag, with the text that follows it up to the next tag.
struct Tag<'a> {
    name: String,
    closing: bool,
    value: &'a str,
    offset: usize,
}

fn tags(text: &str) -> Vec<Tag<'_>> {
    let mut tags = Vec::new();
    let mut rest = 0;
    while let Some(start) = text[rest..].find('<').map(|i| rest + i) {
        let Some(end) = text[start..].find('>').map(|i| start + i) else {
            break;
        };
        let raw = &text[start + 1..end];
        let next = text[end + 1..]
            .find('<')
            .map_or(text.len(), |i| end + 1 + i);
        // Processing instructions and comments of OFX 2.x.
        if !raw.starts_with(['?', '!']) {
            let (closing, name) = match raw.strip_prefix('/') {
                Some(name) => (true, name),
                None => (false, raw),
            };
            tags.push(Tag {
                name: name.trim().to_ascii_uppercase(),
                closing,
                value: text[end + 1..next].trim(),
                offset: start,
            });
        }
        rest = next;
    }
    tags
}

/// Text of an element, XML entities replaced.
fn unescape(value: &str) -> String {
    let mut text = value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    while let Some(start) = text.find("&#") {
        let Some(end) = text[start..].find(';').map(|i| start + i) else {
            break;
        };
        let code = &text[start + 2..end];
        let decoded = match code.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => code.parse().ok(),
        }
        .and_then(char::from_u32);
        let Some(decoded) = decoded else {
            break;
        };
        text.replace_range(start..=end, &decoded.to_string());
    }
    text.replace("&amp;", "&")
}

/// Day of an OFX date: `YYYYMMDD`, followed maybe by a time and a time zone.
fn date(text: &str, value: &str, offset: usize) -> Result<NaiveDate, InterchangeError> {
    value
        .get(..8)
        .and_then(|day| NaiveDate::parse_from_str(day, "%Y%m%d").ok())
        .ok_or_else(|| InterchangeError::InvalidDate {
            line: line_at(text, offset),
            text: value.to_owned(),
        })
}

/// Fields of a transaction being read.
#[derive(Default)]
struct Transaction {
    offset: usize,
    date: Option<NaiveDate>,
    amount_cents: Option<i64>,
    kind: Option<String>,
    name: Option<String>,
    memo: Option<String>,
}

impl Transaction {
    fn finish(self, text: &str) -> Result<RawLine, InterchangeError> {
        let (Some(date), Some(amount_cents)) = (self.date, self.amount_cents) else {
            return Err(InterchangeError::IncompleteTransaction {
                line: line_at(text, self.offset),
            });
        };
        let memo = self.memo.filter(|memo| !memo.is_empty());
        let (label, note) = match self.name.filter(|name| !name.is_empty()) {
            Some(name) => {
                let note = memo.filter(|memo| *memo != name);
                (name, note)
            }
            None => (memo.or(self.kind).unwrap_or_default(), None),
        };
        Ok(RawLine {
            date,
            value_date: None,
            label,
            amount_cents,
            note,
            source_category: None,
        })
    }
}

/// Reads the bank or card statement of an OFX file: its transactions, its ledger balance
/// (`LEDGERBAL`) as the end balance and its period (`DTSTART`, `DTEND`).
pub fn parse(bytes: &[u8]) -> Result<OfxStatement, InterchangeError> {
    let text = decode(bytes);
    let start = text
        .to_ascii_uppercase()
        .find("<OFX>")
        .ok_or(InterchangeError::NotOfx)?;
    let tags = tags(&text[start..]);
    let opened = |name: &str| tags.iter().filter(|t| !t.closing && t.name == name).count();
    let statements = opened("STMTRS") + opened("CCSTMTRS");
    match statements {
        0 if opened("INVSTMTRS") > 0 => return Err(InterchangeError::InvestmentStatement),
        0 => return Err(InterchangeError::NoAccount),
        1 => {}
        count => {
            return Err(InterchangeError::SeveralAccounts(
                u32::try_from(count).unwrap_or(u32::MAX),
            ));
        }
    }

    let mut lines = Vec::new();
    let mut currency = None;
    let mut end_balance_cents = None;
    let (mut period_start, mut period_end) = (None, None);
    let mut transaction: Option<Transaction> = None;
    // Aggregate whose leaf elements are being read.
    let mut context = "";
    for tag in &tags {
        let offset = start + tag.offset;
        if tag.closing {
            if tag.name == "STMTTRN"
                && let Some(done) = transaction.take()
            {
                lines.push(done.finish(&text)?);
            }
            if tag.name == context {
                context = "";
            }
            continue;
        }
        let value = unescape(tag.value);
        let invalid_amount = || InterchangeError::InvalidAmount {
            line: line_at(&text, offset),
            text: value.clone(),
        };
        match tag.name.as_str() {
            "STMTTRN" => {
                if let Some(done) = transaction.take() {
                    lines.push(done.finish(&text)?);
                }
                transaction = Some(Transaction {
                    offset,
                    ..Transaction::default()
                });
            }
            "LEDGERBAL" => context = "LEDGERBAL",
            "AVAILBAL" => context = "AVAILBAL",
            "BANKTRANLIST" => context = "BANKTRANLIST",
            "CURDEF" => currency = Some(value.to_ascii_uppercase()),
            "DTSTART" if context == "BANKTRANLIST" => {
                period_start = Some(date(&text, &value, offset)?)
            }
            "DTEND" if context == "BANKTRANLIST" => period_end = Some(date(&text, &value, offset)?),
            "BALAMT" if context == "LEDGERBAL" => {
                end_balance_cents = Some(parse_plain_amount(&value).ok_or_else(invalid_amount)?);
            }
            name => {
                if let Some(t) = transaction.as_mut() {
                    match name {
                        "DTPOSTED" => t.date = Some(date(&text, &value, offset)?),
                        "TRNAMT" => {
                            t.amount_cents =
                                Some(parse_plain_amount(&value).ok_or_else(invalid_amount)?);
                        }
                        "TRNTYPE" => t.kind = Some(value),
                        "NAME" => t.name = Some(value),
                        "MEMO" => t.memo = Some(value),
                        _ => {}
                    }
                }
            }
        }
    }
    if let Some(done) = transaction.take() {
        lines.push(done.finish(&text)?);
    }
    lines.retain(|line| line.amount_cents != 0);
    Ok(OfxStatement {
        statement: Statement {
            lines,
            start_balance_cents: None,
            end_balance_cents,
            period: period_start.zip(period_end),
        },
        currency,
    })
}
