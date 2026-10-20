//! Duplicate detection of statement lines against the operations of an account.
//!
//! - **Duplicate**: same date, amount and raw label as an existing operation, imported or
//!   manual. Matches are counted: an existing operation accounts for a single line.
//! - **Ambiguous**, shown to the user and not included by default:
//!   - the line also exists in the account, but fewer times than in the file;
//!   - an operation has the same amount and date with another label;
//!   - a manual entry has the same amount within [`MANUAL_ENTRY_DAYS`] days (manual entries
//!     often precede the import, with another label and date).
//! - **New** otherwise. Identical lines within one file are distinct operations.

use std::collections::{HashMap, HashSet};

use chrono::{Days, NaiveDate};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::RawLine;
use crate::error::AppResult;

/// Maximum gap, in days, between a line and a manual entry of the same amount.
pub const MANUAL_ENTRY_DAYS: u64 = 3;

/// An existing operation of the account, as needed for duplicate detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExistingOperation {
    pub id: i64,
    pub date: NaiveDate,
    pub amount_cents: i64,
    pub raw_label: String,
    pub is_manual: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AmbiguityReason {
    /// The account has this exact operation, but fewer times than the file.
    RepeatedLine,
    /// An operation has the same date and amount, with another label.
    SameDayOtherLabel,
    /// A manual entry has the same amount, a few days apart.
    NearManualEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DuplicateStatus {
    New,
    Duplicate {
        operation_id: i64,
    },
    Ambiguous {
        reason: AmbiguityReason,
        /// Existing operations the line may duplicate.
        candidate_ids: Vec<i64>,
    },
}

impl DuplicateStatus {
    /// Whether the import preview includes the line unless the user unchecks it.
    pub fn included_by_default(&self) -> bool {
        matches!(self, DuplicateStatus::New)
    }
}

/// Status of each line, in the order of `lines`.
pub fn classify(lines: &[RawLine], existing: &[ExistingOperation]) -> Vec<DuplicateStatus> {
    let mut exact: HashMap<(NaiveDate, i64, &str), Vec<i64>> = HashMap::new();
    for operation in existing {
        exact
            .entry((operation.date, operation.amount_cents, &operation.raw_label))
            .or_default()
            .push(operation.id);
    }

    // First pass: exact matches, each existing operation used once, in file order.
    let mut used: HashMap<(NaiveDate, i64, &str), usize> = HashMap::new();
    let mut matched = HashSet::new();
    let mut statuses: Vec<Option<DuplicateStatus>> = lines
        .iter()
        .map(|line| {
            let key = (line.date, line.amount_cents, line.label.as_str());
            let ids = exact.get(&key)?;
            let count = used.entry(key).or_default();
            let status = match ids.get(*count) {
                Some(&id) => {
                    matched.insert(id);
                    DuplicateStatus::Duplicate { operation_id: id }
                }
                None => DuplicateStatus::Ambiguous {
                    reason: AmbiguityReason::RepeatedLine,
                    candidate_ids: ids.clone(),
                },
            };
            *count += 1;
            Some(status)
        })
        .collect();

    // Second pass: lines without exact match, against operations not matched exactly.
    let unmatched: Vec<&ExistingOperation> = existing
        .iter()
        .filter(|operation| !matched.contains(&operation.id))
        .collect();
    for (line, status) in lines.iter().zip(&mut statuses) {
        if status.is_none() {
            *status = Some(near_match(line, &unmatched));
        }
    }
    statuses.into_iter().flatten().collect()
}

fn near_match(line: &RawLine, unmatched: &[&ExistingOperation]) -> DuplicateStatus {
    let same_amount = || {
        unmatched
            .iter()
            .filter(|operation| operation.amount_cents == line.amount_cents)
    };
    let same_day: Vec<i64> = same_amount()
        .filter(|operation| operation.date == line.date)
        .map(|operation| operation.id)
        .collect();
    if !same_day.is_empty() {
        return DuplicateStatus::Ambiguous {
            reason: AmbiguityReason::SameDayOtherLabel,
            candidate_ids: same_day,
        };
    }
    let near_manual: Vec<i64> = same_amount()
        .filter(|operation| {
            operation.is_manual
                && line
                    .date
                    .signed_duration_since(operation.date)
                    .num_days()
                    .unsigned_abs()
                    <= MANUAL_ENTRY_DAYS
        })
        .map(|operation| operation.id)
        .collect();
    if !near_manual.is_empty() {
        return DuplicateStatus::Ambiguous {
            reason: AmbiguityReason::NearManualEntry,
            candidate_ids: near_manual,
        };
    }
    DuplicateStatus::New
}

/// Operations of the account that may match `lines`: those dated within the file period,
/// widened by [`MANUAL_ENTRY_DAYS`] on each side.
pub async fn existing_operations(
    pool: &SqlitePool,
    account_id: i64,
    lines: &[RawLine],
) -> AppResult<Vec<ExistingOperation>> {
    let (Some(first), Some(last)) = (
        lines.iter().map(|line| line.date).min(),
        lines.iter().map(|line| line.date).max(),
    ) else {
        return Ok(Vec::new());
    };
    let margin = Days::new(MANUAL_ENTRY_DAYS);
    let from = first.checked_sub_days(margin).unwrap_or(first);
    let to = last.checked_add_days(margin).unwrap_or(last);
    let operations = sqlx::query_as!(
        ExistingOperation,
        r#"SELECT id AS "id!", date AS "date!: NaiveDate", amount_cents, raw_label,
                  import_batch_id IS NULL AS "is_manual!: bool"
           FROM operation
           WHERE account_id = ?1 AND date BETWEEN ?2 AND ?3
           ORDER BY date, id"#,
        account_id,
        from,
        to
    )
    .fetch_all(pool)
    .await?;
    Ok(operations)
}

#[cfg(test)]
mod tests;
