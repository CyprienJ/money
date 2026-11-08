//! Balance check of a statement: `start balance + Σ lines = end balance`.
//!
//! The sum covers every line read from the statement, duplicates and lines before the
//! account opening included: the check tells whether the statement was read completely, not
//! what is imported.

use serde::Serialize;
use specta::Type;

use super::RawLine;
use super::batch::BalanceCheckStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BalanceCheck {
    pub status: BalanceCheckStatus,
    /// Sum of the statement lines.
    pub lines_total_cents: i64,
    /// Start balance plus the lines; `None` without a start balance.
    pub computed_end_cents: Option<i64>,
}

/// Checks the lines against the balances printed on the statement. Without both balances
/// (CSV exports), the check is not available.
pub fn check(start_cents: Option<i64>, lines: &[RawLine], end_cents: Option<i64>) -> BalanceCheck {
    let lines_total_cents = lines.iter().map(|line| line.amount_cents).sum();
    let computed_end_cents = start_cents.map(|start| start + lines_total_cents);
    let status = match (computed_end_cents, end_cents) {
        (Some(computed), Some(end)) if computed == end => BalanceCheckStatus::Ok,
        (Some(_), Some(_)) => BalanceCheckStatus::Mismatch,
        _ => BalanceCheckStatus::NotAvailable,
    };
    BalanceCheck {
        status,
        lines_total_cents,
        computed_end_cents,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn lines(amounts: &[i64]) -> Vec<RawLine> {
        amounts
            .iter()
            .map(|&amount_cents| RawLine {
                date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                value_date: None,
                label: "X".to_owned(),
                amount_cents,
                note: None,
                source_category: None,
                splits: Vec::new(),
                source_page: None,
                source_line: None,
            })
            .collect()
    }

    #[test]
    fn balanced_statement_is_ok() {
        assert_eq!(
            check(Some(797), &lines(&[-80, 8_000]), Some(8_717)),
            BalanceCheck {
                status: BalanceCheckStatus::Ok,
                lines_total_cents: 7_920,
                computed_end_cents: Some(8_717),
            }
        );
        // No line: the balances must be equal.
        assert_eq!(
            check(Some(500), &[], Some(500)).status,
            BalanceCheckStatus::Ok
        );
    }

    #[test]
    fn a_missing_or_misread_line_is_a_mismatch() {
        let result = check(Some(797), &lines(&[-80]), Some(8_717));
        assert_eq!(result.status, BalanceCheckStatus::Mismatch);
        assert_eq!(result.computed_end_cents, Some(717));
    }

    #[test]
    fn without_both_balances_the_check_is_not_available() {
        let result = check(None, &lines(&[-80]), None);
        assert_eq!(result.status, BalanceCheckStatus::NotAvailable);
        assert_eq!(
            (result.lines_total_cents, result.computed_end_cents),
            (-80, None)
        );
        assert_eq!(
            check(Some(0), &[], None).status,
            BalanceCheckStatus::NotAvailable
        );
        assert_eq!(
            check(None, &[], Some(0)).status,
            BalanceCheckStatus::NotAvailable
        );
    }
}
