//! Scheduled operations (rent, salary, subscriptions): display only in V1. Their upcoming
//! occurrences are computed on the fly and never create operations; real operations come
//! from imports.
//!
//! `day_of_period` is the weekday for weekly schedules (1 = Monday … 7 = Sunday) and the day
//! of the month otherwise (1 to 31, moved to the last day of shorter months). Quarterly,
//! semiannual and yearly schedules repeat from the month of their start date.

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::account;
use crate::error::{AppResult, ErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Frequency {
    Weekly,
    Monthly,
    Quarterly,
    Semiannual,
    Yearly,
}

impl Frequency {
    /// Months between two occurrences; `None` for weekly schedules.
    fn months(self) -> Option<u32> {
        match self {
            Frequency::Weekly => None,
            Frequency::Monthly => Some(1),
            Frequency::Quarterly => Some(3),
            Frequency::Semiannual => Some(6),
            Frequency::Yearly => Some(12),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledOperation {
    pub id: i64,
    pub account_id: i64,
    pub label: String,
    /// Negative for a debit, positive for a credit.
    pub amount_cents: i64,
    pub category_id: Option<i64>,
    pub frequency: Frequency,
    pub day_of_period: i64,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
}

/// Fields of a scheduled operation to create or update.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledOperationInput {
    pub account_id: i64,
    pub label: String,
    pub amount_cents: i64,
    pub category_id: Option<i64>,
    pub frequency: Frequency,
    pub day_of_period: i64,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
}

/// An upcoming occurrence of a scheduled operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingOperation {
    pub scheduled_operation_id: i64,
    pub date: NaiveDate,
    pub account_id: i64,
    pub label: String,
    pub amount_cents: i64,
    pub category_id: Option<i64>,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScheduleError {
    #[error("scheduled operation {0} not found")]
    NotFound(i64),

    #[error("the label cannot be empty")]
    EmptyLabel,

    #[error("the amount cannot be zero")]
    ZeroAmount,

    #[error(
        "the day must be a weekday from 1 (Monday) to 7 for a weekly schedule, a day from 1 to 31 otherwise"
    )]
    InvalidDay,

    #[error("the end date cannot precede the start date")]
    EndBeforeStart,

    #[error("investment, asset and liability accounts have no operations")]
    AccountWithoutOperations,

    #[error("category {0} not found")]
    CategoryNotFound(i64),
}

impl ScheduleError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            ScheduleError::NotFound(_) | ScheduleError::CategoryNotFound(_) => ErrorKind::NotFound,
            _ => ErrorKind::Validation,
        }
    }
}

/// Occurrences of a schedule between `from` and `to` (included), within its own dates.
pub fn occurrences(
    schedule: &ScheduledOperation,
    from: NaiveDate,
    to: NaiveDate,
) -> Vec<NaiveDate> {
    let first = from.max(schedule.start_date);
    let last = schedule.end_date.map_or(to, |end| end.min(to));
    if first > last {
        return Vec::new();
    }
    match schedule.frequency.months() {
        None => {
            // First matching weekday on or after `first`, then every 7 days.
            let weekday = schedule.day_of_period as u32;
            let offset = (weekday + 7 - first.weekday().number_from_monday()) % 7;
            std::iter::successors(first.checked_add_days(Days::new(offset.into())), |d| {
                d.checked_add_days(Days::new(7))
            })
            .take_while(|date| *date <= last)
            .collect()
        }
        Some(interval) => {
            let anchor =
                NaiveDate::from_ymd_opt(schedule.start_date.year(), schedule.start_date.month(), 1)
                    .expect("first day of a month exists");
            std::iter::successors(Some(anchor), |month| {
                month.checked_add_months(Months::new(interval))
            })
            .map(|month| day_in_month(month, schedule.day_of_period as u32))
            .take_while(|date| *date <= last)
            .filter(|date| *date >= first)
            .collect()
        }
    }
}

/// `day` of the month starting on `first_day`, or its last day when the month is shorter.
fn day_in_month(first_day: NaiveDate, day: u32) -> NaiveDate {
    let last_day = first_day
        .checked_add_months(Months::new(1))
        .and_then(|next| next.pred_opt())
        .expect("months have a last day");
    first_day
        .with_day(day.min(last_day.day()))
        .expect("day within the month")
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<ScheduledOperation>> {
    let schedules = sqlx::query_as!(
        ScheduledOperation,
        r#"SELECT id AS "id!", account_id, label, amount_cents, category_id,
                  frequency AS "frequency: Frequency", day_of_period,
                  start_date AS "start_date: NaiveDate", end_date AS "end_date: NaiveDate"
           FROM scheduled_operation
           ORDER BY label COLLATE NOCASE"#
    )
    .fetch_all(pool)
    .await?;
    Ok(schedules)
}

async fn get(pool: &SqlitePool, id: i64) -> AppResult<ScheduledOperation> {
    let schedule = sqlx::query_as!(
        ScheduledOperation,
        r#"SELECT id AS "id!", account_id, label, amount_cents, category_id,
                  frequency AS "frequency: Frequency", day_of_period,
                  start_date AS "start_date: NaiveDate", end_date AS "end_date: NaiveDate"
           FROM scheduled_operation
           WHERE id = ?1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(schedule.ok_or(ScheduleError::NotFound(id))?)
}

pub async fn create(
    pool: &SqlitePool,
    input: ScheduledOperationInput,
) -> AppResult<ScheduledOperation> {
    let label = check(pool, &input).await?;
    let id = sqlx::query!(
        "INSERT INTO scheduled_operation
         (account_id, label, amount_cents, category_id, frequency, day_of_period, start_date, end_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        input.account_id,
        label,
        input.amount_cents,
        input.category_id,
        input.frequency,
        input.day_of_period,
        input.start_date,
        input.end_date
    )
    .execute(pool)
    .await?
    .last_insert_rowid();
    get(pool, id).await
}

pub async fn update(
    pool: &SqlitePool,
    id: i64,
    input: ScheduledOperationInput,
) -> AppResult<ScheduledOperation> {
    get(pool, id).await?;
    let label = check(pool, &input).await?;
    sqlx::query!(
        "UPDATE scheduled_operation
         SET account_id = ?2, label = ?3, amount_cents = ?4, category_id = ?5, frequency = ?6,
             day_of_period = ?7, start_date = ?8, end_date = ?9
         WHERE id = ?1",
        id,
        input.account_id,
        label,
        input.amount_cents,
        input.category_id,
        input.frequency,
        input.day_of_period,
        input.start_date,
        input.end_date
    )
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let deleted = sqlx::query!("DELETE FROM scheduled_operation WHERE id = ?1", id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(ScheduleError::NotFound(id).into());
    }
    Ok(())
}

/// Occurrences of every schedule from `from` over `days` days (`from` included), by date.
pub async fn upcoming(
    pool: &SqlitePool,
    from: NaiveDate,
    days: u32,
) -> AppResult<Vec<UpcomingOperation>> {
    let to = from
        .checked_add_days(Days::new(u64::from(days.max(1)) - 1))
        .unwrap_or(from);
    let mut upcoming: Vec<UpcomingOperation> = list(pool)
        .await?
        .iter()
        .flat_map(|schedule| {
            occurrences(schedule, from, to)
                .into_iter()
                .map(|date| UpcomingOperation {
                    scheduled_operation_id: schedule.id,
                    date,
                    account_id: schedule.account_id,
                    label: schedule.label.clone(),
                    amount_cents: schedule.amount_cents,
                    category_id: schedule.category_id,
                })
        })
        .collect();
    upcoming.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.label.cmp(&b.label)));
    Ok(upcoming)
}

/// Checks the input and returns the trimmed label.
async fn check(pool: &SqlitePool, input: &ScheduledOperationInput) -> AppResult<String> {
    let label = input.label.trim();
    if label.is_empty() {
        return Err(ScheduleError::EmptyLabel.into());
    }
    if input.amount_cents == 0 {
        return Err(ScheduleError::ZeroAmount.into());
    }
    let max_day = if input.frequency == Frequency::Weekly {
        7
    } else {
        31
    };
    if !(1..=max_day).contains(&input.day_of_period) {
        return Err(ScheduleError::InvalidDay.into());
    }
    if input.end_date.is_some_and(|end| end < input.start_date) {
        return Err(ScheduleError::EndBeforeStart.into());
    }
    let account = account::get(pool, input.account_id).await?;
    if !account.account_type.has_operations() {
        return Err(ScheduleError::AccountWithoutOperations.into());
    }
    if let Some(category_id) = input.category_id {
        let exists = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!: i64" FROM category WHERE id = ?1"#,
            category_id
        )
        .fetch_one(pool)
        .await?;
        if exists == 0 {
            return Err(ScheduleError::CategoryNotFound(category_id).into());
        }
    }
    Ok(label.to_owned())
}

#[cfg(test)]
mod tests;
