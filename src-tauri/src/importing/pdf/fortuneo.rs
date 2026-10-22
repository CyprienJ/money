//! Fortuneo current account statements ("Relevé de Compte").
//!
//! Each operation is a line `DD/MM  DD/MM/YYYY  label  amount`, the amount sitting under the
//! "Débit" or "Crédit" header; the label may go on over the next lines. The statement gives
//! the closing date ("Arrêté au 30 septembre 2026") and the opening and closing balances
//! ("ANCIEN SOLDE CRÉDITEUR AU 31 AOÛT 2026 … 7,97 €", "NOUVEAU SOLDE …").

use chrono::{Datelike, NaiveDate};

use super::french::{is_amount, parse_amount, parse_long_date};
use super::text::{TextItem, TextLine};
use super::{DocumentText, PdfError, Statement, StatementParser};
use crate::importing::RawLine;

pub struct Fortuneo;

/// Maximum gap, in points, between the lines of one operation label.
const CONTINUATION_GAP: f32 = 16.0;

/// Text further left than this, in points, from the "Date" header is in the page margin.
const MARGIN_GAP: f32 = 12.0;

/// Position of the operations table header.
struct Columns {
    /// Left of the "Date" header: text left of it is in the margin ("Réf : MRELPAR", the
    /// issue date), printed on the same baseline as some table lines.
    date: f32,
    /// Left of the "Crédit" header: amounts starting there are credits.
    credit: f32,
}

impl StatementParser for Fortuneo {
    fn id(&self) -> &'static str {
        "fortuneo"
    }

    fn bank(&self) -> &'static str {
        "Fortuneo"
    }

    fn recognizes(&self, document: &DocumentText) -> bool {
        document.contains("FORTUNEO") && document.contains("Relevé de Compte")
    }

    fn parse(&self, document: &DocumentText) -> Result<Statement, PdfError> {
        let end = closing_date(document)?;
        let mut start_balance = None;
        let mut end_balance = None;
        let mut start_date = None;
        let mut lines: Vec<RawLine> = Vec::new();

        for page in &document.pages {
            let mut columns = None;
            // y and label start of the last operation line, while its label may go on.
            let mut last: Option<(f32, f32)> = None;
            for line in &page.lines {
                if let Some(header) = table_header(line) {
                    columns = Some(header);
                    continue;
                }
                let Some(columns) = &columns else { continue };
                let Some(line) = without_margin(line, columns) else {
                    continue;
                };
                let line = &line;
                let text = line.text();

                if text.starts_with("ANCIEN SOLDE") {
                    start_balance = Some(balance(line)?);
                    start_date = balance_date(line).and_then(|d| d.succ_opt());
                    last = None;
                } else if text.starts_with("NOUVEAU SOLDE") {
                    end_balance = Some(balance(line)?);
                    last = None;
                } else if text.starts_with("TOTAL") || text.starts_with("SOLDE") {
                    last = None;
                } else if let Some((operation, label_left)) = operation(line, columns, end)? {
                    lines.push(operation);
                    last = Some((line.y, label_left));
                } else if let (Some((y, label_left)), Some(previous)) = (last, lines.last_mut()) {
                    // Label going on under the previous one, without date nor amount.
                    let under_label = (line.items[0].left - label_left).abs() <= 4.0;
                    if under_label && line.y - y <= CONTINUATION_GAP {
                        previous.label = format!("{} {}", previous.label, text);
                        last = Some((line.y, label_left));
                    } else {
                        last = None;
                    }
                }
            }
        }

        if lines.is_empty() && start_balance.is_none() {
            return Err(PdfError::Layout("no operations table found".to_owned()));
        }
        Ok(Statement {
            lines,
            start_balance_cents: start_balance,
            end_balance_cents: end_balance,
            period: start_date.map(|start| (start, end)),
        })
    }
}

/// "Arrêté au 30 septembre 2026".
fn closing_date(document: &DocumentText) -> Result<NaiveDate, PdfError> {
    document
        .lines()
        .find_map(|line| {
            let text = line.text();
            let words: Vec<&str> = text.split_whitespace().collect();
            let at = words.iter().position(|w| *w == "Arrêté")?;
            match words.get(at + 1..at + 5)? {
                ["au", day, month, year] => parse_long_date(day, month, year),
                _ => None,
            }
        })
        .ok_or_else(|| PdfError::Layout("closing date (\"Arrêté au …\") not found".to_owned()))
}

fn table_header(line: &TextLine) -> Option<Columns> {
    let find = |name: &str| line.items.iter().find(|item| item.text == name);
    let date = find("Date")?;
    let label = find("Opération")?;
    let credit = find("Crédit")?;
    find("Débit")?;
    (date.left < label.left).then_some(Columns {
        date: date.left,
        credit: credit.left,
    })
}

/// The line without its page margin text, or `None` when nothing else is left.
fn without_margin(line: &TextLine, columns: &Columns) -> Option<TextLine> {
    let items: Vec<TextItem> = line
        .items
        .iter()
        .filter(|item| item.left >= columns.date - MARGIN_GAP)
        .cloned()
        .collect();
    (!items.is_empty()).then_some(TextLine { y: line.y, items })
}

/// Signed balance of an "ANCIEN SOLDE" or "NOUVEAU SOLDE" line: the amount before `€`,
/// negative when the balance is "DÉBITEUR".
fn balance(line: &TextLine) -> Result<i64, PdfError> {
    let amount = line
        .items
        .iter()
        .rev()
        .find(|item| is_amount(&item.text))
        .and_then(|item| parse_amount(&item.text))
        .ok_or_else(|| PdfError::Layout(format!("no amount in \"{}\"", line.text())))?;
    Ok(if line.text().contains("DÉBITEUR") {
        -amount
    } else {
        amount
    })
}

/// "… AU 31 AOÛT 2026" at the end of a balance line.
fn balance_date(line: &TextLine) -> Option<NaiveDate> {
    let text = line.text();
    let words: Vec<&str> = text.split_whitespace().collect();
    let at = words.iter().rposition(|w| *w == "AU")?;
    match words.get(at + 1..at + 4)? {
        [day, month, year] => parse_long_date(day, month, year),
        _ => None,
    }
}

/// An operation line with the left of its label, or `None` when the line does not start
/// with a `DD/MM` date.
fn operation(
    line: &TextLine,
    columns: &Columns,
    end: NaiveDate,
) -> Result<Option<(RawLine, f32)>, PdfError> {
    let mut items = line.items.iter();
    let Some(date) = items.next().and_then(|item| day_month(&item.text, end)) else {
        return Ok(None);
    };
    let rest: Vec<&TextItem> = items.collect();
    let value_date = rest
        .first()
        .and_then(|item| NaiveDate::parse_from_str(&item.text, "%d/%m/%Y").ok());
    let (amount_item, label_items) = match rest.split_last() {
        Some((last, others)) if is_amount(&last.text) => (*last, others),
        _ => {
            return Err(PdfError::Layout(format!(
                "no amount in operation \"{}\"",
                line.text()
            )));
        }
    };
    let label_items = &label_items[usize::from(value_date.is_some())..];
    let Some(first) = label_items.first() else {
        return Err(PdfError::Layout(format!(
            "no label in operation \"{}\"",
            line.text()
        )));
    };
    let label = label_items
        .iter()
        .map(|item| item.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let amount = parse_amount(&amount_item.text).expect("checked by is_amount");
    let amount_cents = if amount_item.left >= columns.credit - 4.0 {
        amount
    } else {
        -amount
    };
    let operation = RawLine {
        date,
        value_date,
        label,
        amount_cents,
        note: None,
        source_category: None,
    };
    Ok(Some((operation, first.left)))
}

/// `DD/MM` within the statement ending on `end`: a month after the closing month belongs to
/// the previous year.
fn day_month(text: &str, end: NaiveDate) -> Option<NaiveDate> {
    let (day, month) = text.split_once('/')?;
    if day.len() != 2 || month.len() != 2 {
        return None;
    }
    let (day, month): (u32, u32) = (day.parse().ok()?, month.parse().ok()?);
    let year = if month > end.month() {
        end.year() - 1
    } else {
        end.year()
    };
    NaiveDate::from_ymd_opt(year, month, day)
}

#[cfg(test)]
mod tests {
    use super::super::text::PageText;
    use super::*;

    /// A line of items given as `(left, text)`.
    fn line(y: f32, items: &[(f32, &str)]) -> TextLine {
        TextLine {
            y,
            items: items
                .iter()
                .map(|&(left, text)| TextItem {
                    left,
                    right: left + 5.0 * text.len() as f32,
                    text: text.to_owned(),
                })
                .collect(),
        }
    }

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    #[test]
    fn dates_belong_to_the_statement_year() {
        let end = date("2026-01-31");
        assert_eq!(day_month("28/12", end), Some(date("2025-12-28")));
        assert_eq!(day_month("05/01", end), Some(date("2026-01-05")));
        assert_eq!(day_month("5/01", end), None);
        assert_eq!(day_month("08/09/2026", end), None);
    }

    #[test]
    fn ignores_the_margin_text_on_table_lines() {
        let lines = vec![
            line(37.0, &[(264.0, "Relevé de Compte"), (534.0, "FORTUNEO")]),
            line(73.0, &[(264.0, "Arrêté au 30 avril 2025")]),
            line(
                360.0,
                &[
                    (57.0, "Date"),
                    (84.0, "Date de Valeur"),
                    (237.0, "Opération"),
                    (408.0, "Débit"),
                    (496.0, "Crédit"),
                ],
            ),
            line(
                379.0,
                &[(54.0, "ANCIEN SOLDE"), (529.0, "20,00"), (550.0, "€")],
            ),
            line(
                655.0,
                &[
                    (30.0, "Réf : MRELPAR"),
                    (54.0, "30/04"),
                    (90.0, "30/04/2025"),
                    (136.0, "CARTE 28/04 SUSHI PARIS"),
                    (444.0, "15,90"),
                ],
            ),
            line(
                695.0,
                &[
                    (30.0, "01/05/2025"),
                    (54.0, "NOUVEAU SOLDE CRÉDITEUR AU 30 AVRIL 2025"),
                    (520.0, "4,10"),
                    (550.0, "€"),
                ],
            ),
        ];
        let document = DocumentText {
            pages: vec![PageText {
                width: 595.0,
                height: 842.0,
                lines,
            }],
        };

        let statement = Fortuneo.parse(&document).unwrap();
        assert_eq!(
            statement.lines,
            [RawLine {
                date: date("2025-04-30"),
                value_date: Some(date("2025-04-30")),
                label: "CARTE 28/04 SUSHI PARIS".to_owned(),
                amount_cents: -1590,
                note: None,
                source_category: None,
            }]
        );
        assert_eq!(statement.start_balance_cents, Some(2000));
        assert_eq!(statement.end_balance_cents, Some(410));
    }
}
