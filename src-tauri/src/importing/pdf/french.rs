//! French dates and amounts as written in bank statements.

use chrono::NaiveDate;

use crate::importing::csv::{DecimalSeparator, parse_cents};

/// Month number of a French month name, full or abbreviated, in any case, with or without
/// accents and final dot: `septembre`, `SEPT.`, `août`, `AOUT`, `févr.`.
pub fn month_number(name: &str) -> Option<u32> {
    let name = name.trim_end_matches('.').to_lowercase();
    let name = name.replace(['é', 'è', 'ê'], "e").replace('û', "u");
    let prefixes = [
        ("janv", 1),
        ("fev", 2),
        ("mars", 3),
        ("avr", 4),
        ("mai", 5),
        ("juin", 6),
        ("juil", 7),
        ("aou", 8),
        ("sep", 9),
        ("oct", 10),
        ("nov", 11),
        ("dec", 12),
    ];
    prefixes
        .iter()
        .find(|(prefix, _)| name.starts_with(prefix) && name.len() >= 3)
        .map(|&(_, month)| month)
}

/// Date written as `30 septembre 2026` or `19 oct. 2026`.
pub fn parse_long_date(day: &str, month: &str, year: &str) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year.parse().ok()?, month_number(month)?, day.parse().ok()?)
}

/// Amount written with a decimal comma, such as `1 234,56 €`, in cents.
pub fn parse_amount(text: &str) -> Option<i64> {
    parse_cents(text, DecimalSeparator::Comma).ok().flatten()
}

/// Whether the text is an amount with exactly two decimals, as in statement columns.
pub fn is_amount(text: &str) -> bool {
    let digits = text.trim_end_matches('€').trim_end();
    digits.len() > 3 && digits.as_bytes()[digits.len() - 3] == b',' && parse_amount(text).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_french_month_names() {
        for (name, month) in [
            ("janvier", 1),
            ("JANV.", 1),
            ("févr.", 2),
            ("FÉVRIER", 2),
            ("mars", 3),
            ("avr.", 4),
            ("mai", 5),
            ("juin", 6),
            ("juil.", 7),
            ("AOÛT", 8),
            ("aout", 8),
            ("sept.", 9),
            ("septembre", 9),
            ("oct.", 10),
            ("nov.", 11),
            ("déc.", 12),
            ("DÉCEMBRE", 12),
        ] {
            assert_eq!(month_number(name), Some(month), "{name}");
        }
        assert_eq!(month_number("ma"), None);
        assert_eq!(month_number("lundi"), None);
    }

    #[test]
    fn reads_long_dates_and_amounts() {
        assert_eq!(
            parse_long_date("30", "septembre", "2026"),
            NaiveDate::from_ymd_opt(2026, 9, 30)
        );
        assert_eq!(parse_long_date("31", "févr.", "2026"), None);
        assert_eq!(parse_amount("1 234,56 €"), Some(123_456));
        assert_eq!(parse_amount("-178,43 €"), Some(-17_843));
        assert!(is_amount("0,80") && is_amount("4200,58 €"));
        assert!(!is_amount("2026") && !is_amount("08/09") && !is_amount("12,5"));
    }
}
