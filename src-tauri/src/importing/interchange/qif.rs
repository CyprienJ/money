//! QIF files, as exported by Microsoft Money (one account per export) or Quicken: sections
//! opened by a `!Type:` line, records of one field per line (`D` date, `T` amount, `P` payee,
//! `M` memo, `L` category, `S`/`E`/`$` splits) closed by `^`.
//!
//! Dates are written in the order of the language of the software: the order day/month is
//! deduced from the file (a part above 12), day first by default as in the French versions.
//! A two-digit year is in the 1900s from 70, in the 2000s below, or after an apostrophe
//! (`31/12' 0`). The first record of a Money export, the opening balance, is not a
//! transaction: it gives the start balance.

use chrono::NaiveDate;

use super::{InterchangeError, decode, parse_grouped_amount};
use crate::importing::{RawLine, RawSplit, Statement};

/// Payees of the opening balance record written by Money and Quicken.
const OPENING_BALANCE_PAYEES: [&str; 4] = [
    "opening balance",
    "solde d'ouverture",
    "solde initial",
    "solde de départ",
];

/// Kind of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    /// Transactions of a bank, cash, card, asset or liability account.
    Transactions,
    Investment,
    /// Lists (categories, classes, accounts, memorized transactions…): skipped.
    List,
}

fn section_of(header: &str) -> Section {
    let Some(kind) = header
        .strip_prefix("!Type:")
        .or_else(|| header.strip_prefix("!type:"))
    else {
        return Section::List;
    };
    match kind.trim().to_lowercase().as_str() {
        "bank" | "cash" | "ccard" | "oth a" | "oth l" | "banque" | "liquide" | "carte" => {
            Section::Transactions
        }
        "invst" | "invest" => Section::Investment,
        _ => Section::List,
    }
}

/// A transaction as written in the file.
#[derive(Debug, Default)]
struct Record {
    line: u32,
    date: Option<(u32, String)>,
    amount: Option<(u32, String)>,
    payee: Option<String>,
    memo: Option<String>,
    category: Option<String>,
    splits: Vec<Split>,
}

/// A split of a transaction: its category, and the line and text of its amount.
#[derive(Debug)]
struct Split {
    category: Option<String>,
    amount: Option<(u32, String)>,
}

/// Category of an `L` or `S` field: the class after "/" removed; `None` when empty.
fn category(text: &str) -> Option<String> {
    let category = match text.split_once('/') {
        // A transfer `[Compte]` may hold a "/" in the account name.
        Some((category, _)) if !text.starts_with('[') => category,
        _ => text,
    };
    let category = category.trim();
    (!category.is_empty()).then(|| category.to_owned())
}

fn is_transfer(category: &str) -> bool {
    category.starts_with('[') && category.ends_with(']')
}

/// Reads the transactions of the only account of a QIF file.
pub fn parse(bytes: &[u8]) -> Result<Statement, InterchangeError> {
    let text = decode(bytes);
    if !text.trim_start().starts_with('!') {
        return Err(InterchangeError::NotQif);
    }

    // Records of each section with transactions, in file order.
    let mut sections: Vec<(Section, Vec<Record>)> = Vec::new();
    let mut current = Section::List;
    let mut record = Record::default();
    for (index, raw) in text.lines().enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with('!') {
            current = section_of(line.trim());
            if current != Section::List {
                sections.push((current, Vec::new()));
            }
            record = Record::default();
            continue;
        }
        if current == Section::List {
            continue;
        }
        if record.line == 0 {
            record.line = number;
        }
        let (field, value) = line.split_at(line.chars().next().map_or(0, char::len_utf8));
        let value = value.trim();
        match field {
            "^" => {
                let done = std::mem::take(&mut record);
                if done.date.is_some() || done.amount.is_some() {
                    sections
                        .last_mut()
                        .expect("records are in a section")
                        .1
                        .push(done);
                }
            }
            "D" => record.date = Some((number, value.to_owned())),
            "T" | "U" => {
                if record.amount.is_none() {
                    record.amount = Some((number, value.to_owned()));
                }
            }
            "P" => record.payee = Some(value.to_owned()),
            "M" => record.memo = Some(value.to_owned()),
            "L" => record.category = category(value),
            "S" => record.splits.push(Split {
                category: category(value),
                amount: None,
            }),
            "$" => {
                if let Some(split) = record.splits.last_mut() {
                    split.amount = Some((number, value.to_owned()));
                }
            }
            _ => {}
        }
    }

    let with_records: Vec<&(Section, Vec<Record>)> = sections
        .iter()
        .filter(|(_, records)| !records.is_empty())
        .collect();
    let records = match with_records.as_slice() {
        [] => return Err(InterchangeError::NoAccount),
        [(Section::Investment, _)] => return Err(InterchangeError::InvestmentStatement),
        [(_, records)] => records,
        several => {
            return Err(InterchangeError::SeveralAccounts(
                u32::try_from(several.len()).unwrap_or(u32::MAX),
            ));
        }
    };

    let day_first = day_first(records.iter().filter_map(|r| r.date.as_ref()));
    let mut lines = Vec::new();
    let mut start_balance_cents = None;
    for (index, record) in records.iter().enumerate() {
        let (Some((date_line, date_text)), Some((amount_line, amount_text))) =
            (&record.date, &record.amount)
        else {
            return Err(InterchangeError::IncompleteTransaction { line: record.line });
        };
        let date =
            parse_date(date_text, day_first).ok_or_else(|| InterchangeError::InvalidDate {
                line: *date_line,
                text: date_text.clone(),
            })?;
        let amount_cents = amount(*amount_line, amount_text)?;
        if index == 0 && is_opening_balance(record) {
            start_balance_cents = Some(amount_cents);
            continue;
        }
        if amount_cents == 0 {
            continue;
        }
        let (source_category, splits_note, splits) = split_details(record, amount_cents)?;
        let memo = record.memo.clone().filter(|memo| !memo.is_empty());
        let payee = record.payee.clone().filter(|payee| !payee.is_empty());
        let (label, memo) = match (payee, memo) {
            (Some(payee), memo) => (payee, memo),
            (None, Some(memo)) => (memo, None),
            // Money exports transfers without payee: their category ("[Livret A]") names them.
            (None, None) => (
                source_category
                    .clone()
                    .unwrap_or_else(|| "Sans libellé".to_owned()),
                None,
            ),
        };
        let note = [memo, splits_note]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        lines.push(RawLine {
            date,
            value_date: None,
            label,
            amount_cents,
            note: (!note.is_empty()).then(|| note.join(" · ")),
            source_category,
            splits,
            source_page: None,
            source_line: Some(record.line),
        });
    }
    Ok(Statement {
        lines,
        start_balance_cents,
        end_balance_cents: None,
        period: None,
    })
}

fn amount(line: u32, text: &str) -> Result<i64, InterchangeError> {
    parse_grouped_amount(text).ok_or_else(|| InterchangeError::InvalidAmount {
        line,
        text: text.to_owned(),
    })
}

fn is_opening_balance(record: &Record) -> bool {
    let payee = record
        .payee
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    record.category.as_deref().is_some_and(is_transfer)
        && OPENING_BALANCE_PAYEES.contains(&payee.as_str())
}

/// Details of a transaction of `total` cents: its category, the detail of its splits for the
/// note, and its parts. Splits consistent with the total, in several categories, are kept as
/// parts, without note. Otherwise, a split transaction keeps a category only when every split
/// has the same, and its splits are detailed in the note.
type Details = (Option<String>, Option<String>, Vec<RawSplit>);

fn split_details(record: &Record, total: i64) -> Result<Details, InterchangeError> {
    if record.splits.is_empty() {
        return Ok((record.category.clone(), None, Vec::new()));
    }
    let mut amounts = Vec::new();
    for split in &record.splits {
        amounts.push(match &split.amount {
            Some((line, text)) => Some(amount(*line, text)?),
            None => None,
        });
    }
    let first = &record.splits[0].category;
    let same_category = record.splits.iter().all(|split| split.category == *first);
    let parts: Option<Vec<RawSplit>> = record
        .splits
        .iter()
        .zip(&amounts)
        .map(|(split, amount)| {
            let amount_cents = (*amount)?;
            (amount_cents != 0 && amount_cents.signum() == total.signum()).then(|| RawSplit {
                source_category: split.category.clone(),
                amount_cents,
            })
        })
        .collect();
    if let Some(parts) = parts
        && parts.len() >= 2
        && !same_category
        && parts.iter().map(|part| part.amount_cents).sum::<i64>() == total
    {
        return Ok((None, None, parts));
    }

    let detail: Vec<String> = record
        .splits
        .iter()
        .zip(&amounts)
        .map(|(split, amount)| part_text(split.category.as_deref(), *amount))
        .collect();
    let shared = same_category.then(|| first.clone()).flatten();
    Ok((shared, Some(format!("Ventilation : {}", detail.join(" ; "))), Vec::new()))
}

/// "Ventilation : Alimentation -12,00 ; Loisirs -8,00", the note of a split line imported as
/// a single operation.
pub(crate) fn split_note(parts: &[RawSplit]) -> String {
    let detail: Vec<String> = parts
        .iter()
        .map(|part| part_text(part.source_category.as_deref(), Some(part.amount_cents)))
        .collect();
    format!("Ventilation : {}", detail.join(" ; "))
}

fn part_text(category: Option<&str>, amount_cents: Option<i64>) -> String {
    let name = category.unwrap_or("Sans catégorie");
    match amount_cents {
        Some(cents) => format!("{name} {}", format_cents(cents)),
        None => name.to_owned(),
    }
}

/// "-12,50" for the note of a split transaction.
fn format_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{sign}{},{:02}", abs / 100, abs % 100)
}

/// Day, month and year parts of a date, and whether the year follows an apostrophe.
fn date_parts(text: &str) -> Option<([u32; 3], bool)> {
    let apostrophe = text.contains('\'');
    let parts: Vec<&str> = text.split(['/', '.', '-', '\'']).map(str::trim).collect();
    let [a, b, c] = parts.as_slice() else {
        return None;
    };
    let number = |part: &str| -> Option<u32> {
        (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse().ok())
            .flatten()
    };
    Some(([number(a)?, number(b)?, number(c)?], apostrophe))
}

/// Whether the dates are written day first: a first part above 12 says so, a second part
/// above 12 says the contrary; day first without a clue.
fn day_first<'a>(dates: impl Iterator<Item = &'a (u32, String)>) -> bool {
    for (_, text) in dates {
        let Some(([first, second, _], _)) = date_parts(text) else {
            continue;
        };
        // ISO dates start with the year.
        if first > 31 {
            continue;
        }
        if first > 12 {
            return true;
        }
        if second > 12 {
            return false;
        }
    }
    true
}

fn parse_date(text: &str, day_first: bool) -> Option<NaiveDate> {
    let ([first, second, third], apostrophe) = date_parts(text)?;
    if first > 31 {
        return NaiveDate::from_ymd_opt(i32::try_from(first).ok()?, second, third);
    }
    let (day, month) = if day_first {
        (first, second)
    } else {
        (second, first)
    };
    let year = match third {
        0..=99 if apostrophe => 2000 + third,
        0..=69 => 2000 + third,
        70..=99 => 1900 + third,
        year => year,
    };
    NaiveDate::from_ymd_opt(i32::try_from(year).ok()?, month, day)
}
