//! Matching of operations with the occurrences of scheduled operations. An operation realizes
//! an occurrence when it is on the same account, in the same direction, with an amount within
//! `AMOUNT_TOLERANCE_PERCENT` and a date within `DETECTION_WINDOW_DAYS` of the occurrence.
//!
//! A match is certain when the occurrence and the operation are each the only candidate of the
//! other: certain matches are made automatically, the others are proposed to the user.

use std::collections::{HashMap, HashSet};

use chrono::{Days, NaiveDate};
use serde::Serialize;
use specta::Type;
use sqlx::{SqliteConnection, SqlitePool};

use super::{
    MatchedOperation, ScheduleError, ScheduledOperation, UpcomingOperation, list, occurrences,
};
use crate::error::AppResult;
use crate::operation::OperationError;

#[cfg(test)]
mod tests;

/// Largest gap between the amount of an occurrence and its operation, in percent of the
/// scheduled amount.
pub const AMOUNT_TOLERANCE_PERCENT: i64 = 10;

/// Largest gap, in days, between an occurrence and a detected operation.
pub const DETECTION_WINDOW_DAYS: u64 = 5;

/// Largest gap, in days, between an occurrence and an operation the user matches by hand.
pub const MANUAL_WINDOW_DAYS: i64 = 15;

/// A detected match the user has to confirm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleMatchProposal {
    pub occurrence: UpcomingOperation,
    pub operation: MatchedOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Candidate {
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
    operation_id: i64,
}

/// Whether `amount_cents` is in the same direction as `scheduled_cents`, within the tolerance.
pub(super) fn amount_matches(scheduled_cents: i64, amount_cents: i64) -> bool {
    scheduled_cents.signum() == amount_cents.signum()
        && (amount_cents - scheduled_cents).abs() * 100
            <= scheduled_cents.abs() * AMOUNT_TOLERANCE_PERCENT
}

/// Candidate matches of the unrealized occurrences up to `today` plus the detection window,
/// with the unmatched operations, without the dismissed ones.
async fn candidates(pool: &SqlitePool, today: NaiveDate) -> AppResult<Vec<Candidate>> {
    let window = Days::new(DETECTION_WINDOW_DAYS);
    let horizon = today.checked_add_days(window).unwrap_or(today);
    let realized: HashSet<(i64, NaiveDate)> = sqlx::query!(
        r#"SELECT scheduled_operation_id, occurrence_date AS "occurrence_date: NaiveDate"
           FROM schedule_match"#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| (row.scheduled_operation_id, row.occurrence_date))
    .collect();
    let dismissed: HashSet<Candidate> = sqlx::query!(
        r#"SELECT scheduled_operation_id, occurrence_date AS "occurrence_date: NaiveDate",
                  operation_id
           FROM schedule_match_dismissal"#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| Candidate {
        scheduled_operation_id: row.scheduled_operation_id,
        occurrence_date: row.occurrence_date,
        operation_id: row.operation_id,
    })
    .collect();

    let mut candidates = Vec::new();
    for schedule in list(pool).await? {
        let dates: Vec<NaiveDate> = occurrences(&schedule, schedule.start_date, horizon)
            .into_iter()
            .filter(|date| !realized.contains(&(schedule.id, *date)))
            .collect();
        let (Some(first), Some(last)) = (dates.first(), dates.last()) else {
            continue;
        };
        let from = first.checked_sub_days(window).unwrap_or(*first);
        let to = last.checked_add_days(window).unwrap_or(*last);
        let operations = sqlx::query!(
            r#"SELECT o.id AS "id!", o.date AS "date!: NaiveDate", o.amount_cents
               FROM operation o
               WHERE o.account_id = ?1 AND o.date BETWEEN ?2 AND ?3
                 AND NOT EXISTS (SELECT 1 FROM schedule_match m WHERE m.operation_id = o.id)"#,
            schedule.account_id,
            from,
            to
        )
        .fetch_all(pool)
        .await?;
        for date in &dates {
            for operation in &operations {
                let gap = (operation.date - *date).num_days().unsigned_abs();
                let candidate = Candidate {
                    scheduled_operation_id: schedule.id,
                    occurrence_date: *date,
                    operation_id: operation.id,
                };
                if gap <= DETECTION_WINDOW_DAYS
                    && amount_matches(schedule.amount_cents, operation.amount_cents)
                    && !dismissed.contains(&candidate)
                {
                    candidates.push(candidate);
                }
            }
        }
    }
    Ok(candidates)
}

/// Splits the candidates into certain matches (each side has no other candidate) and the
/// others.
fn split_certain(candidates: &[Candidate]) -> (Vec<Candidate>, Vec<Candidate>) {
    let mut occurrences: HashMap<(i64, NaiveDate), usize> = HashMap::new();
    let mut operations: HashMap<i64, usize> = HashMap::new();
    for c in candidates {
        *occurrences
            .entry((c.scheduled_operation_id, c.occurrence_date))
            .or_default() += 1;
        *operations.entry(c.operation_id).or_default() += 1;
    }
    candidates.iter().partition(|c| {
        occurrences[&(c.scheduled_operation_id, c.occurrence_date)] == 1
            && operations[&c.operation_id] == 1
    })
}

/// Makes the certain matches; with `batch_id`, only those of operations of that import.
/// Returns their count.
pub async fn match_certain(
    pool: &SqlitePool,
    today: NaiveDate,
    batch_id: Option<i64>,
) -> AppResult<u32> {
    let (certain, _) = split_certain(&candidates(pool, today).await?);
    let mut tx = pool.begin().await?;
    let mut matched = 0;
    for c in certain {
        if let Some(batch_id) = batch_id {
            let in_batch = sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!: i64" FROM operation
                   WHERE id = ?1 AND import_batch_id = ?2"#,
                c.operation_id,
                batch_id
            )
            .fetch_one(&mut *tx)
            .await?;
            if in_batch == 0 {
                continue;
            }
        }
        insert(
            &mut tx,
            c.scheduled_operation_id,
            c.occurrence_date,
            c.operation_id,
        )
        .await?;
        matched += 1;
    }
    tx.commit().await?;
    Ok(matched)
}

/// Detected matches that are not certain, by occurrence date.
pub async fn proposals(
    pool: &SqlitePool,
    today: NaiveDate,
) -> AppResult<Vec<ScheduleMatchProposal>> {
    let (_, mut uncertain) = split_certain(&candidates(pool, today).await?);
    uncertain.sort_by_key(|c| (c.occurrence_date, c.scheduled_operation_id, c.operation_id));
    let schedules: HashMap<i64, ScheduledOperation> =
        list(pool).await?.into_iter().map(|s| (s.id, s)).collect();
    let mut conn = pool.acquire().await?;
    let mut proposals = Vec::with_capacity(uncertain.len());
    for c in uncertain {
        let schedule = &schedules[&c.scheduled_operation_id];
        proposals.push(ScheduleMatchProposal {
            occurrence: occurrence(schedule, c.occurrence_date),
            operation: matched_operation(&mut conn, c.operation_id).await?,
        });
    }
    Ok(proposals)
}

/// Records that an operation realizes an occurrence. The date must be an occurrence of the
/// schedule, the operation on its account; neither may be matched already.
pub async fn match_occurrence(
    pool: &SqlitePool,
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
    operation_id: i64,
) -> AppResult<()> {
    let schedule = check_occurrence(pool, scheduled_operation_id, occurrence_date).await?;
    let mut tx = pool.begin().await?;
    let account_id = sqlx::query_scalar!(
        "SELECT account_id FROM operation WHERE id = ?1",
        operation_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(OperationError::NotFound(operation_id))?;
    if account_id != schedule.account_id {
        return Err(ScheduleError::OtherAccount.into());
    }
    let realized = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM schedule_match
           WHERE scheduled_operation_id = ?1 AND occurrence_date = ?2"#,
        scheduled_operation_id,
        occurrence_date
    )
    .fetch_one(&mut *tx)
    .await?;
    if realized > 0 {
        return Err(ScheduleError::AlreadyRealized.into());
    }
    let used = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM schedule_match WHERE operation_id = ?1"#,
        operation_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if used > 0 {
        return Err(ScheduleError::OperationAlreadyMatched(operation_id).into());
    }
    insert(
        &mut tx,
        scheduled_operation_id,
        occurrence_date,
        operation_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Forgets the operation that realized an occurrence.
pub async fn unmatch(
    pool: &SqlitePool,
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM schedule_match WHERE scheduled_operation_id = ?1 AND occurrence_date = ?2",
        scheduled_operation_id,
        occurrence_date
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Dismisses a proposed match, which is not proposed again.
pub async fn dismiss(
    pool: &SqlitePool,
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
    operation_id: i64,
) -> AppResult<()> {
    super::get(pool, scheduled_operation_id).await?;
    crate::operation::get(pool, operation_id).await?;
    sqlx::query!(
        "INSERT OR IGNORE INTO schedule_match_dismissal
         (scheduled_operation_id, occurrence_date, operation_id) VALUES (?1, ?2, ?3)",
        scheduled_operation_id,
        occurrence_date,
        operation_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Operations the user may match by hand with an occurrence: unmatched, on the account of the
/// schedule, in the same direction, at most `MANUAL_WINDOW_DAYS` apart, whatever their amount;
/// the closest dates first.
pub async fn match_candidates(
    pool: &SqlitePool,
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
) -> AppResult<Vec<MatchedOperation>> {
    let schedule = check_occurrence(pool, scheduled_operation_id, occurrence_date).await?;
    let sign = schedule.amount_cents.signum();
    let ids = sqlx::query_scalar!(
        r#"SELECT o.id AS "id!"
           FROM operation o
           WHERE o.account_id = ?1
             AND (o.amount_cents > 0) = (?2 > 0)
             AND ABS(julianday(o.date) - julianday(?3)) <= ?4
             AND NOT EXISTS (SELECT 1 FROM schedule_match m WHERE m.operation_id = o.id)
           ORDER BY ABS(julianday(o.date) - julianday(?3)), ABS(o.amount_cents - ?5), o.id"#,
        schedule.account_id,
        sign,
        occurrence_date,
        MANUAL_WINDOW_DAYS,
        schedule.amount_cents
    )
    .fetch_all(pool)
    .await?;
    let mut conn = pool.acquire().await?;
    let mut operations = Vec::with_capacity(ids.len());
    for id in ids {
        operations.push(matched_operation(&mut conn, id).await?);
    }
    Ok(operations)
}

/// Operations that realized the occurrences between `from` and `to`, by occurrence.
pub(crate) async fn realized_between(
    pool: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
) -> AppResult<HashMap<(i64, NaiveDate), MatchedOperation>> {
    let rows = sqlx::query!(
        r#"SELECT m.scheduled_operation_id, m.occurrence_date AS "occurrence_date: NaiveDate",
                  o.id, o.date AS "date: NaiveDate", o.amount_cents,
                  COALESCE(p.name, o.normalized_label) AS "payee_name!: String"
           FROM schedule_match m
           JOIN operation o ON o.id = m.operation_id
           LEFT JOIN payee p ON p.id = o.payee_id
           WHERE m.occurrence_date BETWEEN ?1 AND ?2"#,
        from,
        to
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                (row.scheduled_operation_id, row.occurrence_date),
                MatchedOperation {
                    operation_id: row.id,
                    date: row.date,
                    amount_cents: row.amount_cents,
                    payee_name: row.payee_name,
                },
            )
        })
        .collect())
}

/// Removes the matches a schedule change made invalid: dates that are no longer occurrences,
/// operations on another account.
pub(crate) async fn prune(pool: &SqlitePool, schedule: &ScheduledOperation) -> AppResult<()> {
    let matches = sqlx::query!(
        r#"SELECT m.occurrence_date AS "occurrence_date: NaiveDate", o.account_id
           FROM schedule_match m
           JOIN operation o ON o.id = m.operation_id
           WHERE m.scheduled_operation_id = ?1"#,
        schedule.id
    )
    .fetch_all(pool)
    .await?;
    for row in matches {
        let date = row.occurrence_date;
        if row.account_id != schedule.account_id || !is_occurrence(schedule, date) {
            unmatch(pool, schedule.id, date).await?;
        }
    }
    Ok(())
}

fn is_occurrence(schedule: &ScheduledOperation, date: NaiveDate) -> bool {
    occurrences(schedule, date, date).contains(&date)
}

async fn check_occurrence(
    pool: &SqlitePool,
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
) -> AppResult<ScheduledOperation> {
    let schedule = super::get(pool, scheduled_operation_id).await?;
    if !is_occurrence(&schedule, occurrence_date) {
        return Err(ScheduleError::NotAnOccurrence(occurrence_date).into());
    }
    Ok(schedule)
}

/// An occurrence of `schedule`, not realized.
pub(super) fn occurrence(schedule: &ScheduledOperation, date: NaiveDate) -> UpcomingOperation {
    UpcomingOperation {
        scheduled_operation_id: schedule.id,
        date,
        account_id: schedule.account_id,
        label: schedule.label.clone(),
        amount_cents: schedule.amount_cents,
        category_id: schedule.category_id,
        realized_by: None,
    }
}

async fn insert(
    conn: &mut SqliteConnection,
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
    operation_id: i64,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO schedule_match (scheduled_operation_id, occurrence_date, operation_id)
         VALUES (?1, ?2, ?3)",
        scheduled_operation_id,
        occurrence_date,
        operation_id
    )
    .execute(conn)
    .await?;
    Ok(())
}

async fn matched_operation(conn: &mut SqliteConnection, id: i64) -> AppResult<MatchedOperation> {
    let operation = sqlx::query_as!(
        MatchedOperation,
        r#"SELECT o.id AS operation_id, o.date AS "date: NaiveDate", o.amount_cents,
                  COALESCE(p.name, o.normalized_label) AS "payee_name!: String"
           FROM operation o
           LEFT JOIN payee p ON p.id = o.payee_id
           WHERE o.id = ?1"#,
        id
    )
    .fetch_optional(conn)
    .await?;
    Ok(operation.ok_or(OperationError::NotFound(id))?)
}
