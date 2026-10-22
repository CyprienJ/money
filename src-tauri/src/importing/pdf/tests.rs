use std::path::PathBuf;

use chrono::NaiveDate;

use super::text::{Segment, TextLine, group_lines};
use super::*;
use crate::importing::RawLine;
use crate::investment::TransactionKind;

/// PDFium downloaded by `tools/fetch_pdfium.sh`.
fn library_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/pdfium"))
}

fn fixture(name: &str) -> DocumentText {
    let path = format!("{}/../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
    extract(&library_dir(), &bytes)
        .unwrap_or_else(|error| panic!("{error} (run tools/fetch_pdfium.sh)"))
}

fn segment(top: f32, left: f32, text: &str) -> Segment {
    Segment {
        top,
        bottom: top + 10.0,
        left,
        right: left + 8.0 * text.len() as f32,
        text: text.to_owned(),
    }
}

#[test]
fn groups_segments_into_lines_in_reading_order() {
    let lines = group_lines(vec![
        segment(100.5, 300.0, "0,80"),
        segment(50.0, 10.0, "Relevé"),
        segment(100.0, 10.0, "08/09"),
        segment(99.0, 80.0, "CARTE MAXICOFFEE"),
        segment(120.0, 10.0, "Suite"),
    ]);
    let texts: Vec<String> = lines.iter().map(TextLine::text).collect();
    assert_eq!(texts, ["Relevé", "08/09 CARTE MAXICOFFEE 0,80", "Suite"]);
    assert_eq!(lines[1].items[0].left, 10.0);
    assert!(lines[0].y < lines[1].y && lines[1].y < lines[2].y);
}

#[test]
fn extracts_the_fortuneo_statement_lines() {
    let document = fixture("fortuneo.pdf");
    assert_eq!(document.pages.len(), 1);
    let page = &document.pages[0];
    assert!(page.width > 590.0 && page.height > 840.0);

    assert!(document.contains("FORTUNEO"));
    let operation = document
        .lines()
        .find(|line| line.text().contains("MAXICOFFEE"))
        .expect("card payment line");
    // Date, value date, label and debit on one line, from left to right.
    let texts: Vec<&str> = operation.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts.first(), Some(&"08/09"));
    assert_eq!(texts.last(), Some(&"0,80"));
    assert!(operation.items.windows(2).all(|w| w[0].left <= w[1].left));

    let balance = document
        .lines()
        .find(|line| line.text().contains("NOUVEAU SOLDE"))
        .expect("closing balance line");
    assert!(balance.text().ends_with("87,17 €"), "{}", balance.text());
    assert!(balance.y > operation.y);
}

#[test]
fn extracts_every_page_of_the_trade_republic_statement() {
    let document = fixture("trade_republic.pdf");
    assert_eq!(document.pages.len(), 5);
    assert!(document.contains("TRADE REPUBLIC"));
    assert!(
        document
            .lines()
            .filter(|l| l.text().contains("Versement PEA"))
            .count()
            >= 10
    );
}

#[test]
fn rejects_files_that_are_not_pdfs() {
    let error = extract(&library_dir(), b"Date;Label;Amount").unwrap_err();
    assert!(matches!(error, PdfError::InvalidPdf(_)), "{error:?}");
}

struct FakeParser;

impl StatementParser for FakeParser {
    fn id(&self) -> &'static str {
        "fake"
    }
    fn bank(&self) -> &'static str {
        "Banque factice"
    }
    fn recognizes(&self, document: &DocumentText) -> bool {
        document.contains("FORTUNEO")
    }
    fn parse(&self, _: &DocumentText) -> Result<Statement, PdfError> {
        Err(PdfError::Layout("not implemented".to_owned()))
    }
}

#[test]
fn detects_the_parser_from_the_content() {
    static FAKE: FakeParser = FakeParser;
    let parsers: &[&'static dyn StatementParser] = &[&FAKE];
    let fortuneo = fixture("fortuneo.pdf");
    let detected = detect_among(parsers, &fortuneo).expect("recognized");
    assert_eq!(SupportedBank::of(detected).name, "Banque factice");
    assert!(detect_among(parsers, &fixture("trade_republic.pdf")).is_none());
    assert!(matches!(
        detected.parse(&fortuneo),
        Err(PdfError::Layout(_))
    ));
}

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn raw(day: &str, value_date: Option<&str>, label: &str, amount_cents: i64) -> RawLine {
    RawLine {
        date: date(day),
        value_date: value_date.map(date),
        label: label.to_owned(),
        amount_cents,
        note: None,
        source_category: None,
    }
}

#[test]
fn parses_the_fortuneo_statement() {
    let document = fixture("fortuneo.pdf");
    let parser = detect(&document).expect("Fortuneo recognized");
    assert_eq!(parser.id(), "fortuneo");
    let statement = parser.parse(&document).unwrap();
    assert_eq!(
        statement.lines,
        vec![
            raw(
                "2026-09-08",
                Some("2026-09-08"),
                "CARTE 07/09 MAXICOFFEE SUD SATURNE",
                -80
            ),
            raw(
                "2026-09-24",
                Some("2026-09-24"),
                "VIR INST Captain Hadock Envoye depuis Revolut 1234567891234567891",
                8_000,
            ),
        ]
    );
    assert_eq!(statement.start_balance_cents, Some(797));
    assert_eq!(statement.end_balance_cents, Some(8_717));
    assert_eq!(
        statement.period,
        Some((date("2026-09-01"), date("2026-09-30")))
    );
    // The statement balances: start + Σ = end.
    let total: i64 = statement.lines.iter().map(|l| l.amount_cents).sum();
    assert_eq!(797 + total, 8_717);
}

#[test]
fn reads_the_fortuneo_lines_sharing_a_baseline_with_the_margin() {
    // The last operation and the closing balance are on the baseline of the margin text
    // ("Réf : MRELPAR", issue date).
    let document = fixture("fortuneo_2.pdf");
    let parser = detect(&document).expect("Fortuneo recognized");
    let statement = parser.parse(&document).unwrap();
    assert_eq!(statement.lines.len(), 21);
    assert_eq!(
        statement.lines.last(),
        Some(&raw(
            "2025-04-30",
            Some("2025-04-30"),
            "CARTE 28/04 SC-M'SUSHI PARIS",
            -1_590
        ))
    );
    assert_eq!(statement.start_balance_cents, Some(0));
    assert_eq!(statement.end_balance_cents, Some(14_013));
    let total: i64 = statement.lines.iter().map(|l| l.amount_cents).sum();
    assert_eq!(total, 14_013);
}

#[test]
fn parses_the_current_account_of_the_trade_republic_statement() {
    let document = fixture("trade_republic.pdf");
    let parser = detect(&document).expect("Trade Republic recognized");
    assert_eq!(parser.id(), "trade-republic");
    let statement = parser.parse(&document).unwrap();

    assert_eq!(statement.start_balance_cents, Some(6_984));
    assert_eq!(statement.end_balance_cents, Some(6_687));
    assert_eq!(
        statement.period,
        Some((date("2026-07-01"), date("2026-10-19")))
    );
    // PEA trades (pages 4 and 5) are left out.
    assert_eq!(statement.lines.len(), 25);
    assert!(
        statement
            .lines
            .iter()
            .all(|l| !l.label.contains("Buy trade"))
    );

    let lines = &statement.lines;
    assert_eq!(
        lines[0],
        raw("2026-07-01", None, "Intérêts Interest payment", 1)
    );
    assert_eq!(
        lines[1],
        raw("2026-07-03", None, "Virement Versement PEA", -5_920)
    );
    assert_eq!(
        lines[2],
        raw(
            "2026-07-27",
            None,
            "Virement Incoming transfer from Captain Hadock (FR1234567891234567891234567)",
            2_000
        )
    );
    // Description written above the type, IBAN wrapped on the year line.
    assert_eq!(
        lines[20],
        raw(
            "2026-09-27",
            None,
            "Virement Outgoing transfer for CAPTAIN HADOCK (FR1234567891234567891234567)",
            -20_000
        )
    );
    assert_eq!(
        lines[22],
        raw("2026-09-29", None, "Avoir PcComponentes", -64_558)
    );
    assert_eq!(lines[24].date, date("2026-10-19"));

    let money_in: i64 = lines
        .iter()
        .map(|l| l.amount_cents)
        .filter(|a| *a > 0)
        .sum();
    let money_out: i64 = lines
        .iter()
        .map(|l| l.amount_cents)
        .filter(|a| *a < 0)
        .sum();
    assert_eq!((money_in, -money_out), (420_058, 420_355));
}

#[test]
fn parses_the_pea_trades_of_the_trade_republic_statement() {
    let document = fixture("trade_republic.pdf");
    let parser = detect(&document).expect("Trade Republic recognized");
    let trades = parser.parse_trades(&document).unwrap().unwrap();
    let summary: Vec<(NaiveDate, &str, &str, i64, i64)> = trades
        .iter()
        .map(|t| {
            assert_eq!(t.kind, TransactionKind::Buy);
            (
                t.date,
                t.isin.as_str(),
                t.name.as_str(),
                t.quantity_micro,
                t.amount_cents,
            )
        })
        .collect();
    let units = |n: i64| n * 1_000_000;
    assert_eq!(
        summary,
        [
            (
                date("2026-07-03"),
                "FR0014005HJ9",
                "OVH GROUPE (PROM.)EO 1",
                units(4),
                5_920
            ),
            (
                date("2026-07-27"),
                "NL0010273215",
                "ASML HOLDING EO -,09",
                units(1),
                158_200
            ),
            (
                date("2026-07-27"),
                "FR0000120073",
                "AIR LIQUIDE INH. EO 5,50",
                units(1),
                17_843
            ),
            (
                date("2026-07-27"),
                "FR0000121329",
                "THALES S.A. EO 3",
                units(1),
                23_975
            ),
            // Quantity wrapped on the next line.
            (
                date("2026-08-13"),
                "FR0000120172",
                "CARREFOUR S.A. INH.EO 2,5",
                units(6),
                9_538
            ),
            (
                date("2026-08-13"),
                "PLOPTTC00011",
                "CD PROJEKT S.A. C ZY 1",
                units(1),
                6_237
            ),
            (
                date("2026-08-13"),
                "FR0000120404",
                "ACCOR SA INH. EO 3",
                units(3),
                14_142
            ),
            (
                date("2026-08-13"),
                "FR0010208488",
                "ENGIE S.A. INH. EO 1",
                units(10),
                26_294
            ),
            (
                date("2026-08-13"),
                "FR0000124141",
                "VEOLIA ENVIRONNE. EO 5",
                units(5),
                17_305
            ),
            (
                date("2026-08-13"),
                "DE0006231004",
                "INFINEON TECH.AG NA O.N.",
                units(4),
                25_292
            ),
            (
                date("2026-08-13"),
                "FR0000054470",
                "UBISOFT ENTMT IN.EO-,0775",
                units(2),
                1_051
            ),
        ]
    );
    // Fortuneo statements give no trades.
    let fortuneo = fixture("fortuneo.pdf");
    let parser = detect(&fortuneo).expect("Fortuneo recognized");
    assert_eq!(parser.parse_trades(&fortuneo).unwrap(), None);
}
