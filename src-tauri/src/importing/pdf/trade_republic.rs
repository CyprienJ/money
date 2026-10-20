//! Trade Republic statements ("Relevé de compte"), current account section only.
//!
//! A statement may hold several products: the current account ("Compte courant"), then the
//! PEA with its trades. Investment accounts are tracked by valuations (README), so only the
//! current account transactions are read; the summary row gives their opening and closing
//! balances.
//!
//! A transaction spreads over three close baselines: the date (`27 juil.`), then the type,
//! description and amounts, then the year (`2026`); long descriptions wrap. Items are thus
//! grouped around each date. Every row gives the balance after it, which is checked.

use chrono::NaiveDate;

use super::french::{is_amount, parse_amount, parse_long_date};
use super::text::{TextItem, TextLine};
use super::{DocumentText, PdfError, Statement, StatementParser};
use crate::importing::RawLine;

pub struct TradeRepublic;

/// Positions of the transactions table header.
#[derive(Clone, Copy)]
struct Columns {
    date: f32,
    kind: f32,
    description: f32,
    credit: f32,
    debit: f32,
    /// Right edge of the right-aligned balance column.
    balance_right: f32,
}

/// An item with the baseline it sits on.
struct Placed<'a> {
    y: f32,
    item: &'a TextItem,
}

impl StatementParser for TradeRepublic {
    fn id(&self) -> &'static str {
        "trade-republic"
    }

    fn bank(&self) -> &'static str {
        "Trade Republic"
    }

    fn recognizes(&self, document: &DocumentText) -> bool {
        document.contains("TRADE REPUBLIC BANK")
            && document.contains("SYNTHÈSE DU RELEVÉ DE COMPTE")
    }

    fn parse(&self, document: &DocumentText) -> Result<Statement, PdfError> {
        // Summary row: opening balance, money in, money out, closing balance.
        let summary = document
            .lines()
            .find(|line| line.text().starts_with("Compte courant"))
            .ok_or_else(|| PdfError::Layout("no \"Compte courant\" summary".to_owned()))?;
        let amounts: Vec<i64> = amounts(summary).collect();
        let (Some(&start_balance), Some(&end_balance)) = (amounts.first(), amounts.last()) else {
            return Err(PdfError::Layout(format!(
                "summary row \"{}\"",
                summary.text()
            )));
        };
        Ok(Statement {
            lines: transactions(document, start_balance)?,
            start_balance_cents: Some(start_balance),
            end_balance_cents: Some(end_balance),
            period: period(document),
        })
    }
}

/// Transactions of the current account, page by page, each checked against the balance
/// printed after it. The table ends with "APERÇU DU SOLDE" or the summary of another
/// product ("Compte PEA").
fn transactions(document: &DocumentText, start_balance: i64) -> Result<Vec<RawLine>, PdfError> {
    let mut lines = Vec::new();
    let mut balance = start_balance;
    let mut in_current_account = false;

    for page in &document.pages {
        let mut columns: Option<Columns> = None;
        let mut items: Vec<Placed> = Vec::new();
        let mut table_over = false;
        for line in &page.lines {
            let text = line.text();
            if text.starts_with("Compte courant") {
                in_current_account = true;
            } else if text.starts_with("Compte ") && in_current_account {
                in_current_account = false;
                table_over = true;
            } else if !in_current_account || table_over {
                continue;
            } else if let Some(header) = table_header(&page.lines, line) {
                columns = Some(header);
            } else if text.starts_with("APERÇU DU SOLDE") || text.starts_with("Généré le") {
                table_over = true;
            } else if columns.is_some() && !text.contains("D'ARGENT") {
                items.extend(line.items.iter().map(|item| Placed { y: line.y, item }));
            }
        }
        let Some(columns) = columns else { continue };
        for row in group_rows(&items, &columns) {
            let (line, printed_balance) = transaction(&row, &columns)?;
            balance += line.amount_cents;
            if balance != printed_balance {
                return Err(PdfError::Layout(format!(
                    "balance after \"{}\" on {}: {printed_balance} printed, {balance} computed",
                    line.label, line.date
                )));
            }
            lines.push(line);
        }
    }
    Ok(lines)
}

/// "DATE 01 juil. 2026 - 19 oct. 2026" on the first page.
fn period(document: &DocumentText) -> Option<(NaiveDate, NaiveDate)> {
    document.lines().find_map(|line| {
        let text = line.text();
        let (_, dates) = text.split_once("DATE ")?;
        let (from, to) = dates.split_once(" - ")?;
        let date = |text: &str| match text.split_whitespace().collect::<Vec<_>>()[..] {
            [day, month, year] => parse_long_date(day, month, year),
            _ => None,
        };
        Some((date(from)?, date(to.split("IBAN").next()?.trim())?))
    })
}

fn amounts(line: &TextLine) -> impl Iterator<Item = i64> + '_ {
    line.items
        .iter()
        .filter(|item| is_amount(&item.text))
        .filter_map(|item| parse_amount(&item.text))
}

/// The "DATE TYPE DESCRIPTION SOLDE" header, with "ENTRÉE" and "SORTIE" written on a
/// slightly higher baseline.
fn table_header(page: &[TextLine], line: &TextLine) -> Option<Columns> {
    let find =
        |line: &TextLine, name: &str| line.items.iter().find(|item| item.text == name).cloned();
    let date = find(line, "DATE")?;
    let kind = find(line, "TYPE")?;
    let description = find(line, "DESCRIPTION")?;
    let balance = find(line, "SOLDE")?;
    let near = page.iter().filter(|other| (other.y - line.y).abs() < 8.0);
    let (mut credit, mut debit) = (None, None);
    for other in near {
        credit = credit.or_else(|| find(other, "ENTRÉE"));
        debit = debit.or_else(|| find(other, "SORTIE"));
    }
    Some(Columns {
        date: date.left,
        kind: kind.left,
        description: description.left,
        credit: credit?.left,
        debit: debit?.left,
        balance_right: balance.right,
    })
}

/// Items of each transaction: from a little above its date to a little above the next one.
fn group_rows<'a>(items: &'a [Placed<'a>], columns: &Columns) -> Vec<Vec<&'a Placed<'a>>> {
    let is_date = |placed: &Placed| {
        (placed.item.left - columns.date).abs() <= 3.0
            && starts_with_day_and_month(&placed.item.text)
    };
    let mut anchors: Vec<f32> = items.iter().filter(|p| is_date(p)).map(|p| p.y).collect();
    anchors.sort_by(f32::total_cmp);
    anchors
        .iter()
        .enumerate()
        .map(|(index, &y)| {
            let top = y - 2.0;
            let bottom = anchors.get(index + 1).map_or(f32::MAX, |next| next - 2.0);
            let mut row: Vec<&Placed> = items
                .iter()
                .filter(|p| p.y >= top && p.y < bottom)
                .collect();
            row.sort_by(|a, b| {
                a.y.total_cmp(&b.y)
                    .then(a.item.left.total_cmp(&b.item.left))
            });
            row
        })
        .collect()
}

/// `27 juil.`, `13 août` or `03 juil. 2026`.
fn starts_with_day_and_month(text: &str) -> bool {
    let mut words = text.split_whitespace();
    let day = words
        .next()
        .is_some_and(|d| d.len() <= 2 && d.bytes().all(|b| b.is_ascii_digit()));
    let month = words
        .next()
        .is_some_and(|m| super::french::month_number(m).is_some());
    day && month
}

/// A transaction and the balance printed after it.
fn transaction(row: &[&Placed], columns: &Columns) -> Result<(RawLine, i64), PdfError> {
    let in_date = |p: &&&Placed| (p.item.left - columns.date).abs() <= 3.0;
    let date_words: Vec<&str> = row
        .iter()
        .filter(in_date)
        .flat_map(|p| p.item.text.split_whitespace())
        .collect();
    let date = match date_words[..] {
        [day, month, year, ..] => parse_long_date(day, month, year),
        _ => None,
    }
    .ok_or_else(|| PdfError::Layout(format!("transaction date \"{}\"", date_words.join(" "))))?;

    let words = |from: f32, to: f32| {
        row.iter()
            .filter(|p| {
                p.item.left >= from - 3.0 && p.item.left < to - 3.0 && !is_amount(&p.item.text)
            })
            .map(|p| p.item.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let kind = words(columns.kind, columns.description);
    let description = words(columns.description, columns.credit);
    let label = format!("{kind} {description}")
        .replace("( ", "(")
        .replace(" )", ")")
        .trim()
        .to_owned();

    let (mut amount, mut balance) = (None, None);
    for placed in row.iter().filter(|p| is_amount(&p.item.text)) {
        let value = parse_amount(&placed.item.text).expect("checked by is_amount");
        if (placed.item.right - columns.balance_right).abs() <= 4.0 {
            balance = Some(value);
        } else if placed.item.left < columns.debit - 3.0 {
            amount = Some(value);
        } else {
            amount = Some(-value);
        }
    }
    let (Some(amount_cents), Some(balance)) = (amount, balance) else {
        return Err(PdfError::Layout(format!(
            "amount or balance of \"{label}\" on {date}"
        )));
    };
    let line = RawLine {
        date,
        value_date: None,
        label,
        amount_cents,
    };
    Ok((line, balance))
}
