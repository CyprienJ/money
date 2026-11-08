//! Trade Republic statements ("Relevé de compte").
//!
//! A statement may hold several products: the current account ("Compte courant"), then a
//! securities account ("Compte PEA") with its trades. The statement of the current account is
//! its transactions, the summary row giving their opening and closing balances; the trades of
//! the first securities account ("Buy trade FR0000120073 AIR LIQUIDE INH. EO 5,50, quantity:
//! 1") go to the portfolio of an investment account. Its cash movements (payments into the
//! PEA) are not read: the cash account of a PEA is not tracked.
//!
//! A transaction spreads over three close baselines: the date (`27 juil.`), then the type,
//! description and amounts, then the year (`2026`); long descriptions wrap. Items are thus
//! grouped around each date. Every row gives the balance after it, which is checked.

use std::sync::LazyLock;

use chrono::NaiveDate;
use regex::Regex;

use super::french::{is_amount, parse_amount, parse_long_date};
use super::text::{TextItem, TextLine};
use super::{DocumentText, PdfError, Statement, StatementParser, Trade};
use crate::importing::RawLine;
use crate::investment::{QUANTITY_SCALE, TransactionKind};

/// A product of the statement, by the first words of its summary row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Product {
    CurrentAccount,
    /// The first other product: a PEA or a securities account.
    Securities,
}

impl Product {
    fn summary_of(self, text: &str) -> bool {
        match self {
            Product::CurrentAccount => text.starts_with("Compte courant"),
            Product::Securities => {
                text.starts_with("Compte ") && !text.starts_with("Compte courant")
            }
        }
    }
}

/// Description of a trade: kind, ISIN, name and quantity.
static TRADE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(Buy trade|Sell trade|Savings plan execution) ([A-Z]{2}[A-Z0-9]{9}[0-9]) (.+?),?\s*quantity:\s*([0-9]+(?:[.,][0-9]+)?)\s*$",
    )
    .expect("valid regex")
});

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
        let (start_balance, end_balance) = summary(document, Product::CurrentAccount)?
            .ok_or_else(|| PdfError::Layout("no \"Compte courant\" summary".to_owned()))?;
        Ok(Statement {
            lines: transactions(document, Product::CurrentAccount, start_balance)?,
            start_balance_cents: Some(start_balance),
            end_balance_cents: Some(end_balance),
            period: period(document),
        })
    }

    fn parse_trades(&self, document: &DocumentText) -> Result<Option<Vec<Trade>>, PdfError> {
        let Some((start_balance, _)) = summary(document, Product::Securities)? else {
            return Ok(Some(Vec::new()));
        };
        let trades = transactions(document, Product::Securities, start_balance)?
            .iter()
            .filter_map(trade)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(trades))
    }
}

/// Opening and closing balances of the summary row of a product: opening balance, money in,
/// money out, closing balance. `None` when the statement does not hold the product.
fn summary(document: &DocumentText, product: Product) -> Result<Option<(i64, i64)>, PdfError> {
    let Some(summary) = document
        .lines()
        .find(|line| product.summary_of(&line.text()))
    else {
        return Ok(None);
    };
    let amounts: Vec<i64> = amounts(summary).collect();
    match (amounts.first(), amounts.last()) {
        (Some(&start), Some(&end)) => Ok(Some((start, end))),
        _ => Err(PdfError::Layout(format!(
            "summary row \"{}\"",
            summary.text()
        ))),
    }
}

/// The trade of a transaction of the securities account; `None` for a payment.
fn trade(line: &RawLine) -> Option<Result<Trade, PdfError>> {
    let captures = TRADE.captures(&line.label)?;
    let kind = match &captures[1] {
        "Sell trade" => TransactionKind::Sell,
        _ => TransactionKind::Buy,
    };
    let Some(quantity_micro) = quantity_micro(&captures[4]) else {
        return Some(Err(PdfError::Layout(format!(
            "quantity of \"{}\" on {}",
            line.label, line.date
        ))));
    };
    Some(Ok(Trade {
        date: line.date,
        kind,
        isin: captures[2].to_owned(),
        name: captures[3].trim().to_owned(),
        quantity_micro,
        amount_cents: line.amount_cents.abs(),
        source_page: line.source_page,
    }))
}

/// Millionths of a quantity written "4", "0.612345" or "0,5"; `None` beyond 6 decimals.
fn quantity_micro(text: &str) -> Option<i64> {
    let (units, decimals) = text.split_once(['.', ',']).unwrap_or((text, ""));
    if decimals.len() > 6 {
        return None;
    }
    let units: i64 = units.parse().ok()?;
    let decimals: i64 = format!("{decimals:0<6}").parse().ok()?;
    let micro = units.checked_mul(QUANTITY_SCALE)?.checked_add(decimals)?;
    (micro > 0).then_some(micro)
}

/// Transactions of a product, page by page, each checked against the balance printed after
/// it. The table ends with "APERÇU DU SOLDE", the remarks, the footer of the page, or the
/// summary of another product.
fn transactions(
    document: &DocumentText,
    product: Product,
    start_balance: i64,
) -> Result<Vec<RawLine>, PdfError> {
    let mut lines = Vec::new();
    let mut balance = start_balance;
    let mut in_section = false;
    let mut seen = false;

    for (index, page) in document.pages.iter().enumerate() {
        let mut columns: Option<Columns> = None;
        let mut items: Vec<Placed> = Vec::new();
        let mut table_over = false;
        for line in &page.lines {
            let text = line.text();
            if product.summary_of(&text) && !seen {
                in_section = true;
                seen = true;
            } else if text.starts_with("Compte ") && in_section {
                in_section = false;
                table_over = true;
            } else if !in_section || table_over {
                continue;
            } else if let Some(header) = table_header(&page.lines, line) {
                columns = Some(header);
            } else if text.starts_with("APERÇU DU SOLDE")
                || text.starts_with("REMARQUES SUR LE RELEVÉ")
                || text.starts_with("Trade Republic Bank GmbH")
                || text.starts_with("Généré le")
            {
                table_over = true;
            } else if columns.is_some() && !text.contains("D'ARGENT") {
                items.extend(line.items.iter().map(|item| Placed { y: line.y, item }));
            }
        }
        let Some(columns) = columns else { continue };
        for row in group_rows(&items, &columns) {
            let (mut line, printed_balance) = transaction(&row, &columns)?;
            line.source_page = u32::try_from(index + 1).ok();
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
        note: None,
        source_category: None,
        splits: Vec::new(),
        source_page: None,
        source_line: None,
    };
    Ok((line, balance))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(label: &str, amount_cents: i64) -> RawLine {
        RawLine {
            date: NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
            value_date: None,
            label: label.to_owned(),
            amount_cents,
            note: None,
            source_category: None,
            splits: Vec::new(),
            source_page: None,
            source_line: None,
        }
    }

    #[test]
    fn reads_buys_sells_and_savings_plans() {
        let sell = trade(&line(
            "Exécution d'ordre Sell trade NL0010273215 ASML HOLDING EO -,09, quantity: 0.5",
            80_000,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            (sell.kind, sell.isin.as_str(), sell.name.as_str()),
            (
                TransactionKind::Sell,
                "NL0010273215",
                "ASML HOLDING EO -,09"
            )
        );
        assert_eq!((sell.quantity_micro, sell.amount_cents), (500_000, 80_000));

        let plan = trade(&line(
            "Plan d'épargne Savings plan execution IE00B4L5Y983 ISHSIII-CORE MSCI WORLD U.ETF, quantity: 0,612345",
            -5_000,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(plan.kind, TransactionKind::Buy);
        assert_eq!((plan.quantity_micro, plan.amount_cents), (612_345, 5_000));

        assert!(trade(&line("Virement Versement PEA", 5_000)).is_none());
        assert!(matches!(
            trade(&line(
                "Buy trade FR0000120073 AIR LIQUIDE, quantity: 0.1234567",
                -100
            )),
            Some(Err(PdfError::Layout(_)))
        ));
    }
}
