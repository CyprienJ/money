use chrono::NaiveDate;

use super::amount::parse_cents;
use super::*;
use crate::importing::RawLine;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"))
}

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn line(day: &str, label: &str, amount_cents: i64) -> RawLine {
    RawLine {
        date: date(day),
        value_date: None,
        label: label.to_owned(),
        amount_cents,
    }
}

/// Typical French bank export: `;`, Windows-1252, separate debit and credit columns.
fn debit_credit_mapping() -> CsvMapping {
    CsvMapping {
        delimiter: ';',
        encoding: CsvEncoding::Windows1252,
        date_format: "%d/%m/%Y".to_owned(),
        decimal_separator: DecimalSeparator::Comma,
        column_date: 0,
        column_label: 1,
        amount_columns: AmountColumns::DebitCredit {
            debit: 2,
            credit: 3,
        },
        skip_lines: 5,
        column_fee: None,
        state_filter: None,
    }
}

fn signed_mapping() -> CsvMapping {
    CsvMapping {
        delimiter: ';',
        encoding: CsvEncoding::Utf8,
        date_format: "%Y-%m-%d".to_owned(),
        decimal_separator: DecimalSeparator::Dot,
        column_date: 0,
        column_label: 1,
        amount_columns: AmountColumns::Signed { amount: 3 },
        skip_lines: 1,
        column_fee: None,
        state_filter: None,
    }
}

fn parse_text(text: &str, mapping: &CsvMapping) -> Result<Vec<RawLine>, CsvError> {
    parse(text.as_bytes(), mapping)
}

#[test]
fn parses_debit_credit_windows_1252_fixture() {
    let lines = parse(
        &fixture("csv_debit_credit_windows1252.csv"),
        &debit_credit_mapping(),
    )
    .unwrap();
    assert_eq!(
        lines,
        vec![
            line("2026-09-02", "PRLV SEPA EDF CLIENTS PARTICULIERS", -8740),
            line("2026-09-03", "CB CARREFOUR MARKET 01/09", -4512),
            line(
                "2026-09-05",
                "VIR SEPA RECU /DE EMPLOYEUR SA; SALAIRE SEPT",
                245_000
            ),
            line("2026-09-08", "CB Boulangerie Pâtisserie 06/09", -680),
            line("2026-09-12", "PRLV SEPA LOYER SCI DES ÉRABLES", -112_000),
            line("2026-09-15", "REMBOURSEMENT SÉCU CPAM", 2300),
            line("2026-09-28", "VIR VERS LIVRET A", -20_000),
        ]
    );
}

#[test]
fn parses_signed_utf8_fixture_with_bom() {
    let lines = parse(&fixture("csv_signed_utf8_bom.csv"), &signed_mapping()).unwrap();
    assert_eq!(
        lines,
        vec![
            line("2026-09-02", "CARTE X1234 SNCF INTERNET", -5430),
            line("2026-09-04", "VIR INST DE MME MARTIN", 12_000),
            line("2026-09-10", "CARTE X1234 NETFLIX.COM", -1349),
            line("2026-09-30", "INTERETS CREDITEURS", 50),
        ]
    );
}

#[test]
fn iso_8859_1_decodes_like_windows_1252() {
    let mapping = CsvMapping {
        encoding: CsvEncoding::Iso8859_1,
        ..debit_credit_mapping()
    };
    let lines = parse(&fixture("csv_debit_credit_windows1252.csv"), &mapping).unwrap();
    assert_eq!(lines[5].label, "REMBOURSEMENT SÉCU CPAM");
}

#[test]
fn rejects_windows_1252_file_read_as_utf8() {
    let mapping = CsvMapping {
        encoding: CsvEncoding::Utf8,
        ..debit_credit_mapping()
    };
    let error = parse(&fixture("csv_debit_credit_windows1252.csv"), &mapping).unwrap_err();
    assert!(matches!(error, CsvError::InvalidEncoding("UTF-8")));
}

#[test]
fn reports_the_file_line_number() {
    let text = "Date;Label;Amount\n01/09/2026;OK;-1.00\n\n32/09/2026;BAD;-2.00\n";
    let mapping = CsvMapping {
        date_format: "%d/%m/%Y".to_owned(),
        amount_columns: AmountColumns::Signed { amount: 2 },
        ..signed_mapping()
    };
    let error = parse_text(text, &mapping).unwrap_err();
    assert!(
        matches!(&error, CsvError::InvalidDate { line: 4, value } if value == "32/09/2026"),
        "{error:?}"
    );
}

#[test]
fn rejects_lines_without_usable_amount() {
    let mapping = CsvMapping {
        skip_lines: 0,
        ..debit_credit_mapping()
    };
    let cases = [
        ("01/09/2026;X;;", "missing"),
        ("01/09/2026;X;1,00;2,00", "both"),
        ("01/09/2026;X;1,0O;", "invalid"),
        ("01/09/2026;X", "column"),
        ("01/09/2026; ;1,00;", "label"),
    ];
    for (text, expected) in cases {
        let error = parse_text(text, &mapping).unwrap_err();
        let matched = match expected {
            "missing" => matches!(error, CsvError::MissingAmount { line: 1 }),
            "both" => matches!(error, CsvError::BothDebitAndCredit { line: 1 }),
            "invalid" => matches!(error, CsvError::InvalidAmount { line: 1, .. }),
            "column" => matches!(error, CsvError::MissingColumn { line: 1, column: 2 }),
            "label" => matches!(error, CsvError::EmptyLabel { line: 1 }),
            _ => unreachable!(),
        };
        assert!(matched, "{text}: {error:?}");
    }
}

#[test]
fn debit_and_credit_signs_come_from_the_column() {
    let mapping = CsvMapping {
        skip_lines: 0,
        ..debit_credit_mapping()
    };
    let lines = parse_text("01/09/2026;A;-12,00;\n02/09/2026;B;;-3,00\n", &mapping).unwrap();
    assert_eq!(lines[0].amount_cents, -1200);
    assert_eq!(lines[1].amount_cents, 300);
}

#[test]
fn skips_zero_amounts() {
    let mapping = CsvMapping {
        skip_lines: 0,
        ..signed_mapping()
    };
    let lines = parse_text("2026-09-01;X;;0.00\n2026-09-02;Y;;-1.00\n", &mapping).unwrap();
    assert_eq!(lines, vec![line("2026-09-02", "Y", -100)]);

    let mapping = CsvMapping {
        skip_lines: 0,
        ..debit_credit_mapping()
    };
    let lines = parse_text("01/09/2026;X;0,00;0,00\n", &mapping).unwrap();
    assert!(lines.is_empty());
}

#[test]
fn validates_the_mapping() {
    let mapping = CsvMapping {
        delimiter: '€',
        ..signed_mapping()
    };
    assert!(matches!(
        parse_text("", &mapping),
        Err(CsvError::InvalidDelimiter('€'))
    ));
    let mapping = CsvMapping {
        date_format: " ".to_owned(),
        ..signed_mapping()
    };
    assert!(matches!(
        parse_text("", &mapping),
        Err(CsvError::EmptyDateFormat)
    ));
}

#[test]
fn skipping_more_lines_than_the_file_has_gives_no_line() {
    let mapping = CsvMapping {
        skip_lines: 10,
        ..signed_mapping()
    };
    assert_eq!(parse_text("a\nb\n", &mapping).unwrap(), vec![]);
}

#[test]
fn parses_comma_decimal_amounts() {
    let comma = DecimalSeparator::Comma;
    let cases = [
        ("12,34", 1234),
        ("-12,34", -1234),
        ("−12,34", -1234),
        ("12,34-", -1234),
        ("+0,5", 50),
        (",5", 50),
        ("7", 700),
        ("1 234,56", 123_456),
        ("1\u{00A0}234,56", 123_456),
        ("1\u{202F}234\u{202F}567,00", 123_456_700),
        ("1.234,56", 123_456),
        ("1 234,56 €", 123_456),
        ("-45,00 EUR", -4500),
    ];
    for (text, cents) in cases {
        assert_eq!(parse_cents(text, comma), Ok(Some(cents)), "{text}");
    }
}

#[test]
fn parses_dot_decimal_amounts() {
    let dot = DecimalSeparator::Dot;
    assert_eq!(parse_cents("1,234.56", dot), Ok(Some(123_456)));
    assert_eq!(parse_cents("-13.49", dot), Ok(Some(-1349)));
    assert_eq!(parse_cents("0.05", dot), Ok(Some(5)));
}

#[test]
fn empty_amount_is_none() {
    assert_eq!(parse_cents("", DecimalSeparator::Comma), Ok(None));
    assert_eq!(parse_cents("   ", DecimalSeparator::Comma), Ok(None));
}

#[test]
fn rejects_malformed_amounts() {
    let comma = DecimalSeparator::Comma;
    for text in [
        "abc",
        "-",
        ",",
        "1,234",
        "12.50",
        "1.23,00",
        "1234.567,00",
        "1 23,00",
        "1,2,3",
        "--5",
        "12,3a",
        "99999999999999999999",
    ] {
        assert_eq!(parse_cents(text, comma), Err(()), "{text}");
    }
}

#[test]
fn recurring_payees_keep_their_normalized_label_from_one_month_to_the_next() {
    let normalized = |name: &str| -> Vec<String> {
        parse(&fixture(name), &debit_credit_mapping())
            .unwrap()
            .iter()
            .map(|line| crate::labeling::normalize_label(&line.label))
            .collect()
    };
    let september = normalized("csv_debit_credit_windows1252.csv");
    let october = normalized("csv_debit_credit_windows1252_2.csv");
    assert_eq!(october.len(), 5);
    for label in [
        "EDF CLIENTS PARTICULIERS",
        "CARREFOUR MARKET",
        "BOULANGERIE PÂTISSERIE",
        "LOYER SCI DES ÉRABLES",
    ] {
        assert!(
            september.iter().any(|l| l == label),
            "{label} missing in September"
        );
        assert!(
            october.iter().any(|l| l == label),
            "{label} missing in October"
        );
    }
}

#[test]
fn ignores_a_time_after_the_date() {
    // Revolut export: comma separated, ISO date and time, one signed amount column.
    let mapping = CsvMapping {
        delimiter: ',',
        encoding: CsvEncoding::Utf8,
        date_format: "%Y-%m-%d".to_owned(),
        decimal_separator: DecimalSeparator::Dot,
        column_date: 2,
        column_label: 4,
        amount_columns: AmountColumns::Signed { amount: 5 },
        skip_lines: 1,
        column_fee: None,
        state_filter: None,
    };
    let text = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n\
                CARD_PAYMENT,Current,2025-04-01 13:58:02,2025-04-02 09:12:44,Boulangerie,-4.20,0.00,EUR,COMPLETED,120.30\n\
                TOPUP,Current,2025-04-03T08:00:00,2025-04-03 08:00:01,Top-up,50.00,0.00,EUR,COMPLETED,170.30\n";
    let lines = parse_text(text, &mapping).unwrap();
    assert_eq!(
        lines,
        vec![
            line("2025-04-01", "Boulangerie", -420),
            line("2025-04-03", "Top-up", 5_000),
        ]
    );

    // Anything else after the date is still refused.
    let error = parse_text("x\nA,B,2025-04-01 garbage,C,D,-1.00\n", &mapping).unwrap_err();
    assert!(
        matches!(error, CsvError::InvalidDate { line: 2, .. }),
        "{error:?}"
    );
}

#[test]
fn flags_amounts_written_with_the_other_decimal_separator() {
    let mapping = CsvMapping {
        decimal_separator: DecimalSeparator::Comma,
        skip_lines: 0,
        ..signed_mapping()
    };
    let error = parse_text("2026-09-01;X;;500.00", &mapping).unwrap_err();
    assert!(
        matches!(&error, CsvError::WrongDecimalSeparator { line: 1, value } if value == "500.00"),
        "{error:?}"
    );
    // Unreadable either way: a plain invalid amount.
    let error = parse_text("2026-09-01;X;;5O0", &mapping).unwrap_err();
    assert!(
        matches!(error, CsvError::InvalidAmount { line: 1, .. }),
        "{error:?}"
    );
}

/// Revolut export: a fee column and a state column.
fn revolut_mapping() -> CsvMapping {
    CsvMapping {
        delimiter: ',',
        encoding: CsvEncoding::Utf8,
        date_format: "%Y-%m-%d".to_owned(),
        decimal_separator: DecimalSeparator::Dot,
        column_date: 3,
        column_label: 4,
        amount_columns: AmountColumns::Signed { amount: 5 },
        skip_lines: 1,
        column_fee: Some(6),
        state_filter: Some(StateFilter {
            column: 8,
            keep: "COMPLETED".to_owned(),
        }),
    }
}

const REVOLUT: &str = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance
ATM,Current,2025-04-01 10:00:00,2025-04-01 10:00:05,Cash at ATM,-100.00,2.00,EUR,COMPLETED,398.00
CARD_PAYMENT,Current,2025-04-02 12:00:00,,Hotel,-80.00,0.00,EUR,PENDING,
CARD_PAYMENT,Current,2025-04-03 12:00:00,2025-04-04 08:00:00,Shop,-15.00,0.00,EUR,REVERTED,
CARD_PAYMENT,Current,2025-04-05 12:00:00,,Declined,-30.00,0.00,EUR,DECLINED,
EXCHANGE,Current,2025-04-06 09:00:00,2025-04-06 09:00:00,Exchanged to USD,-50.00,0.25,EUR,completed,347.75
";

#[test]
fn takes_fees_off_and_keeps_completed_lines_only() {
    let lines = parse_text(REVOLUT, &revolut_mapping()).unwrap();
    assert_eq!(
        lines,
        vec![
            // 100 € withdrawn plus 2 € of fees.
            line("2025-04-01", "Cash at ATM", -10_200),
            // The state is compared without case.
            line("2025-04-06", "Exchanged to USD", -5_025),
        ]
    );
}

#[test]
fn fees_and_state_are_optional() {
    let mapping = CsvMapping {
        column_fee: None,
        state_filter: None,
        column_date: 2,
        ..revolut_mapping()
    };
    // Every line, fees ignored: the pending ones too.
    assert_eq!(parse_text(REVOLUT, &mapping).unwrap().len(), 5);
}

#[test]
fn rejects_an_empty_state_to_keep() {
    let mapping = CsvMapping {
        state_filter: Some(StateFilter {
            column: 8,
            keep: " ".to_owned(),
        }),
        ..revolut_mapping()
    };
    assert!(matches!(
        parse_text(REVOLUT, &mapping),
        Err(CsvError::EmptyStateValue)
    ));
}
