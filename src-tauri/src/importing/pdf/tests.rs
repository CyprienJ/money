use std::path::PathBuf;

use super::text::{Segment, TextLine, group_lines};
use super::*;

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
