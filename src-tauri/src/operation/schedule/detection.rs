//! Detection of recurring operations the schedule does not cover yet (subscriptions, rent,
//! salary, monthly savings), suggested as scheduled operations.
//!
//! The operations of a payee on an account, in one direction, form a series when, going back
//! from the most recent one, each previous operation comes one period earlier (within the
//! bounds of `Frequency::gap_days`) with an amount within the matching tolerance of the most
//! recent amount. Operations in between that do not fit are skipped; when several fit, the
//! closest amount wins.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::matching::amount_matches;
use super::{Frequency, ScheduledOperation, list};
use crate::error::AppResult;
use crate::payee::PayeeError;

#[cfg(test)]
mod tests;

/// Days after the expected next operation before a series is considered over.
const ACTIVE_GRACE_DAYS: i64 = 10;

impl Frequency {
    /// Smallest and largest gaps, in days, between two operations of a series.
    fn gap_days(self) -> (i64, i64) {
        match self {
            Frequency::Weekly => (6, 8),
            Frequency::Monthly => (26, 36),
            Frequency::Quarterly => (85, 98),
            Frequency::Semiannual => (175, 190),
            Frequency::Yearly => (355, 375),
        }
    }

    /// Usual gap, in days, between two operations.
    fn nominal_days(self) -> i64 {
        match self {
            Frequency::Weekly => 7,
            Frequency::Monthly => 30,
            Frequency::Quarterly => 91,
            Frequency::Semiannual => 182,
            Frequency::Yearly => 365,
        }
    }

    /// Operations needed to recognize a series.
    fn min_count(self) -> usize {
        match self {
            Frequency::Yearly => 2,
            _ => 3,
        }
    }
}

/// Frequencies tried in this order: the shortest period that fits wins.
const FREQUENCIES: [Frequency; 5] = [
    Frequency::Weekly,
    Frequency::Monthly,
    Frequency::Quarterly,
    Frequency::Semiannual,
    Frequency::Yearly,
];

/// A recurring series suggested as a scheduled operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecurringSuggestion {
    pub account_id: i64,
    pub payee_id: i64,
    /// Suggested label of the scheduled operation.
    pub payee_name: String,
    /// Amount of the most recent operation.
    pub amount_cents: i64,
    /// Category of the most recent operation.
    pub category_id: Option<i64>,
    pub frequency: Frequency,
    /// Weekday (1 = Monday) for a weekly series, the usual day of the month otherwise.
    pub day_of_period: i64,
    /// Date of the first operation of the series.
    pub start_date: NaiveDate,
    pub last_date: NaiveDate,
    pub occurrence_count: u32,
}

/// An operation considered for detection.
#[derive(Debug, Clone)]
pub(crate) struct SeriesOperation {
    pub date: NaiveDate,
    pub amount_cents: i64,
    pub category_id: Option<i64>,
    /// Whether the operation already realizes a schedule occurrence.
    pub matched: bool,
}

/// The series ending with the most recent of `operations` (sorted by date), with its frequency,
/// most recent first; `None` when no frequency fits.
pub(crate) fn series(operations: &[SeriesOperation]) -> Option<(Frequency, Vec<&SeriesOperation>)> {
    let latest = operations.last()?;
    FREQUENCIES.into_iter().find_map(|frequency| {
        let (min_gap, max_gap) = frequency.gap_days();
        let mut chain = vec![latest];
        loop {
            let current = chain.last().expect("the chain starts with the latest");
            // The previous operation of the series: in the window one period earlier, the
            // closest amount first, then the gap closest to the usual one.
            let previous = operations
                .iter()
                .filter(|o| {
                    let gap = (current.date - o.date).num_days();
                    (min_gap..=max_gap).contains(&gap)
                        && amount_matches(latest.amount_cents, o.amount_cents)
                })
                .min_by_key(|o| {
                    let gap = (current.date - o.date).num_days();
                    (
                        (o.amount_cents - latest.amount_cents).abs(),
                        (gap - frequency.nominal_days()).abs(),
                    )
                });
            match previous {
                Some(operation) => chain.push(operation),
                None => break,
            }
        }
        (chain.len() >= frequency.min_count()).then_some((frequency, chain))
    })
}

/// Whether a series last seen on `last_date` may still go on at `today`.
fn is_active(frequency: Frequency, last_date: NaiveDate, today: NaiveDate) -> bool {
    (today - last_date).num_days() <= frequency.nominal_days() + ACTIVE_GRACE_DAYS
}

/// Day of the schedule: weekday of the most recent operation for a weekly series, median day
/// of the month otherwise.
fn day_of_period(frequency: Frequency, chain: &[&SeriesOperation]) -> i64 {
    if frequency == Frequency::Weekly {
        return i64::from(chain[0].date.weekday().number_from_monday());
    }
    let mut days: Vec<u32> = chain.iter().map(|o| o.date.day()).collect();
    days.sort_unstable();
    i64::from(days[days.len() / 2])
}

/// Whether a schedule already covers a series of `frequency` on `account_id` around
/// `amount_cents`.
fn covered(
    schedules: &[ScheduledOperation],
    account_id: i64,
    frequency: Frequency,
    amount_cents: i64,
) -> bool {
    schedules.iter().any(|s| {
        s.account_id == account_id
            && s.frequency == frequency
            && amount_matches(s.amount_cents, amount_cents)
    })
}

/// Recurring series that the schedule does not cover and the user did not dismiss, still going
/// on at `today`, by payee name.
pub async fn suggestions(
    pool: &SqlitePool,
    today: NaiveDate,
) -> AppResult<Vec<RecurringSuggestion>> {
    let rows = sqlx::query!(
        r#"SELECT o.account_id, o.payee_id AS "payee_id!", p.name AS payee_name,
                  o.date AS "date: NaiveDate", o.amount_cents,
                  (SELECT CASE WHEN COUNT(*) = 1 THEN MAX(s.category_id) END
                   FROM operation_split s WHERE s.operation_id = o.id) AS "category_id?: i64",
                  EXISTS (SELECT 1 FROM schedule_match m WHERE m.operation_id = o.id)
                      AS "matched!: bool"
           FROM operation o
           JOIN payee p ON p.id = o.payee_id
           WHERE o.date <= ?1
           ORDER BY o.date, o.id"#,
        today
    )
    .fetch_all(pool)
    .await?;
    let dismissed: HashSet<(i64, i64, bool)> = sqlx::query!(
        r#"SELECT account_id, payee_id, is_credit AS "is_credit: bool" FROM recurring_dismissal"#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| (row.account_id, row.payee_id, row.is_credit))
    .collect();
    let schedules = list(pool).await?;

    let mut names: HashMap<i64, String> = HashMap::new();
    let mut groups: HashMap<(i64, i64, bool), Vec<SeriesOperation>> = HashMap::new();
    for row in rows {
        names.entry(row.payee_id).or_insert(row.payee_name);
        groups
            .entry((row.account_id, row.payee_id, row.amount_cents > 0))
            .or_default()
            .push(SeriesOperation {
                date: row.date,
                amount_cents: row.amount_cents,
                category_id: row.category_id,
                matched: row.matched,
            });
    }

    let mut suggestions = Vec::new();
    for ((account_id, payee_id, is_credit), operations) in &groups {
        if dismissed.contains(&(*account_id, *payee_id, *is_credit)) {
            continue;
        }
        let Some((frequency, chain)) = series(operations) else {
            continue;
        };
        let latest = chain[0];
        if !is_active(frequency, latest.date, today)
            || chain.iter().any(|o| o.matched)
            || covered(&schedules, *account_id, frequency, latest.amount_cents)
        {
            continue;
        }
        suggestions.push(RecurringSuggestion {
            account_id: *account_id,
            payee_id: *payee_id,
            payee_name: names[payee_id].clone(),
            amount_cents: latest.amount_cents,
            category_id: latest.category_id,
            frequency,
            day_of_period: day_of_period(frequency, &chain),
            start_date: chain.last().expect("the chain is not empty").date,
            last_date: latest.date,
            occurrence_count: u32::try_from(chain.len()).unwrap_or(u32::MAX),
        });
    }
    suggestions.sort_by(|a, b| {
        a.payee_name
            .to_lowercase()
            .cmp(&b.payee_name.to_lowercase())
            .then(a.account_id.cmp(&b.account_id))
    });
    Ok(suggestions)
}

/// Dismisses the suggestion of a payee's series on an account, in one direction, for good.
pub async fn dismiss(
    pool: &SqlitePool,
    account_id: i64,
    payee_id: i64,
    is_credit: bool,
) -> AppResult<()> {
    crate::account::get(pool, account_id).await?;
    let payee = sqlx::query_scalar!("SELECT id FROM payee WHERE id = ?1", payee_id)
        .fetch_optional(pool)
        .await?;
    if payee.is_none() {
        return Err(PayeeError::NotFound(payee_id).into());
    }
    sqlx::query!(
        "INSERT OR IGNORE INTO recurring_dismissal (account_id, payee_id, is_credit)
         VALUES (?1, ?2, ?3)",
        account_id,
        payee_id,
        is_credit
    )
    .execute(pool)
    .await?;
    Ok(())
}
