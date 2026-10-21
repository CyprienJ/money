use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"))
}

#[test]
fn detects_the_debit_credit_windows_1252_export() {
    assert_eq!(
        detect(&fixture("csv_debit_credit_windows1252.csv")),
        Some(CsvMapping {
            delimiter: ';',
            encoding: CsvEncoding::Windows1252,
            date_format: "%d/%m/%Y".to_owned(),
            decimal_separator: DecimalSeparator::Comma,
            column_date: 0,
            column_label: 1,
            amount_columns: AmountColumns::DebitCredit {
                debit: 2,
                credit: 3
            },
            // Account details, a blank line and the column titles.
            skip_lines: 5,
            column_fee: None,
            state_filter: None,
        })
    );
}

#[test]
fn detects_the_signed_utf8_export() {
    assert_eq!(
        detect(&fixture("csv_signed_utf8_bom.csv")),
        Some(CsvMapping {
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
        })
    );
}

#[test]
fn detects_a_revolut_like_export() {
    let text = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n\
                CARD_PAYMENT,Current,2025-04-01 13:58:02,2025-04-02 09:12:44,Boulangerie,-4.20,0.00,EUR,COMPLETED,120.30\n\
                TOPUP,Current,2025-04-03 08:00:00,2025-04-03 08:00:01,\"Top-up by *1234\",500.00,0.00,EUR,COMPLETED,620.30\n\
                CARD_PAYMENT,Current,2025-04-05 19:30:11,2025-04-06 10:00:00,Restaurant Le Zinc,-38.50,0.00,EUR,COMPLETED,581.80\n";
    assert_eq!(
        detect(text.as_bytes()),
        Some(CsvMapping {
            delimiter: ',',
            encoding: CsvEncoding::Utf8,
            date_format: "%Y-%m-%d".to_owned(),
            decimal_separator: DecimalSeparator::Dot,
            column_date: 2,
            column_label: 4,
            // "Amount", not "Fee" nor "Balance".
            amount_columns: AmountColumns::Signed { amount: 5 },
            skip_lines: 1,
            column_fee: Some(6),
            state_filter: Some(StateFilter {
                column: 8,
                keep: "COMPLETED".to_owned(),
            }),
        })
    );
}

#[test]
fn uses_the_content_without_column_titles() {
    // No header; debit and credit never filled together.
    let text = "12/10/2026;CB CARREFOUR;45,12;\n13/10/2026;VIR SALAIRE;;2 450,00\n14/10/2026;PRLV EDF;87,40;\n";
    assert_eq!(
        detect(text.as_bytes()),
        Some(CsvMapping {
            delimiter: ';',
            encoding: CsvEncoding::Utf8,
            date_format: "%d/%m/%Y".to_owned(),
            decimal_separator: DecimalSeparator::Comma,
            column_date: 0,
            column_label: 1,
            amount_columns: AmountColumns::DebitCredit {
                debit: 2,
                credit: 3
            },
            skip_lines: 0,
            column_fee: None,
            state_filter: None,
        })
    );
}

#[test]
fn finds_nothing_in_files_that_are_not_statements() {
    assert_eq!(detect(b"just some text\nwithout columns\n"), None);
    assert_eq!(detect(b""), None);
    assert_eq!(detect(b"a;b;c\nd;e;f\n"), None);
}

#[test]
fn reads_decimal_marks() {
    assert_eq!(decimal_mark("1 234,56"), Some(','));
    assert_eq!(decimal_mark("-38.5"), Some('.'));
    assert_eq!(decimal_mark("1.234"), None);
    assert_eq!(decimal_mark("2026-10-01"), None);
    assert_eq!(decimal_mark("FNAC.COM"), None);
}
