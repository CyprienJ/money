//! Alerts, computed on demand:
//! - a checking or savings account whose projected balance goes negative within
//!   `FORECAST_DAYS` days (credit cards are negative by nature);
//! - a top-level category whose expenses of the current month exceed by `EXCESS_PERCENT` and
//!   by `MIN_EXCESS_CENTS` their average over the `HISTORY_MONTHS` previous full months, when
//!   the category was spent in `MIN_SPENT_MONTHS` of them at least;
//! - an occurrence of the last `MISSING_LOOKBACK_DAYS` days not realized although the account
//!   has an operation dated more than `matching::DETECTION_WINDOW_DAYS` days after it;
//! - a currency in use without any exchange rate: its amounts count 0 in the statistics.
//!
//! Unusual expenses and missing occurrences can be dismissed; a negative forecast only goes
//! away with the forecast itself.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use crate::account::{self, AccountType};
use crate::currency;
use crate::error::AppResult;
use crate::operation::category::CategoryError;
use crate::operation::schedule::{self, UpcomingOperation, matching};
use crate::stats::{self, BalancePoint, CategoryLevel};

#[cfg(test)]
mod tests;

/// Horizon of the negative forecast alert, in days.
pub const FORECAST_DAYS: u64 = 30;
/// Months the expenses of the current month are compared with.
pub const HISTORY_MONTHS: u32 = 6;
/// Months of the history the category must have been spent in.
pub const MIN_SPENT_MONTHS: usize = 3;
/// Excess over the average, in percent, from which an expense is unusual.
pub const EXCESS_PERCENT: i64 = 50;
/// Smallest excess over the average for an unusual expense.
pub const MIN_EXCESS_CENTS: i64 = 5_000;
/// Days before today in which missing occurrences are looked for.
pub const MISSING_LOOKBACK_DAYS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Alert {
    MissingRate {
        currency: String,
    },
    NegativeForecast {
        account_id: i64,
        /// First day the projected balance is negative.
        first_negative_date: NaiveDate,
        lowest: BalancePoint,
    },
    MissingOccurrence {
        occurrence: UpcomingOperation,
    },
    UnusualExpense {
        category_id: i64,
        /// First day of the current month.
        month: NaiveDate,
        /// Net expenses of the month up to today.
        expense_cents: i64,
        /// Average monthly expenses of the history, rounded.
        average_cents: i64,
    },
}

/// Alerts of `today`: missing rates, negative forecasts by account, missing occurrences by
/// date, then unusual expenses, the largest excess first.
pub async fn list(pool: &SqlitePool, today: NaiveDate) -> AppResult<Vec<Alert>> {
    let mut alerts: Vec<Alert> = currency::in_use(pool)
        .await?
        .into_iter()
        .filter(|use_| use_.latest.is_none())
        .map(|use_| Alert::MissingRate {
            currency: use_.currency,
        })
        .collect();
    alerts.extend(negative_forecasts(pool, today).await?);
    alerts.extend(missing_occurrences(pool, today).await?);
    alerts.extend(unusual_expenses(pool, today).await?);
    Ok(alerts)
}

async fn negative_forecasts(pool: &SqlitePool, today: NaiveDate) -> AppResult<Vec<Alert>> {
    let to = today
        .checked_add_days(Days::new(FORECAST_DAYS))
        .unwrap_or(today);
    let mut alerts = Vec::new();
    for account in account::list(pool, false).await? {
        if !matches!(
            account.account_type,
            AccountType::Checking | AccountType::Savings
        ) {
            continue;
        }
        let forecast = stats::balance_forecast(pool, today, to, Some(&[account.id])).await?;
        if let Some(first) = forecast.points.iter().find(|p| p.balance_cents < 0) {
            alerts.push(Alert::NegativeForecast {
                account_id: account.id,
                first_negative_date: first.date,
                lowest: forecast.lowest,
            });
        }
    }
    Ok(alerts)
}

async fn missing_occurrences(pool: &SqlitePool, today: NaiveDate) -> AppResult<Vec<Alert>> {
    let window = Days::new(matching::DETECTION_WINDOW_DAYS);
    let from = today
        .checked_sub_days(Days::new(MISSING_LOOKBACK_DAYS))
        .unwrap_or(today);
    let accounts: HashSet<i64> = account::list(pool, false)
        .await?
        .into_iter()
        .map(|a| a.id)
        .collect();
    let latest: HashMap<i64, NaiveDate> = sqlx::query!(
        r#"SELECT account_id AS "account_id!", MAX(date) AS "date!: NaiveDate"
           FROM operation
           GROUP BY account_id"#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| (row.account_id, row.date))
    .collect();
    // Occurrences with a proposed match wait for a validation in the schedule.
    let proposed: HashSet<(i64, NaiveDate)> = matching::proposals(pool, today)
        .await?
        .into_iter()
        .map(|p| (p.occurrence.scheduled_operation_id, p.occurrence.date))
        .collect();
    let dismissed: HashSet<(i64, NaiveDate)> = sqlx::query!(
        r#"SELECT scheduled_operation_id, occurrence_date AS "occurrence_date: NaiveDate"
           FROM missing_occurrence_dismissal"#
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| (row.scheduled_operation_id, row.occurrence_date))
    .collect();

    Ok(schedule::occurrences_between(pool, from, today)
        .await?
        .into_iter()
        .filter(|o| {
            let Some(window_end) = o.date.checked_add_days(window) else {
                return false;
            };
            let key = (o.scheduled_operation_id, o.date);
            o.realized_by.is_none()
                && window_end < today
                && accounts.contains(&o.account_id)
                && latest.get(&o.account_id).is_some_and(|d| *d > window_end)
                && !proposed.contains(&key)
                && !dismissed.contains(&key)
        })
        .map(|occurrence| Alert::MissingOccurrence { occurrence })
        .collect())
}

async fn unusual_expenses(pool: &SqlitePool, today: NaiveDate) -> AppResult<Vec<Alert>> {
    let month = first_day_of_month(today);
    let current =
        stats::expenses_by_category(pool, month, today, None, CategoryLevel::TopLevel).await?;

    // Expenses of each category in each month of the history.
    let mut history: HashMap<i64, Vec<i64>> = HashMap::new();
    for back in 1..=HISTORY_MONTHS {
        let start = month
            .checked_sub_months(Months::new(back))
            .expect("months before today exist");
        let end = month
            .checked_sub_months(Months::new(back - 1))
            .and_then(|next| next.pred_opt())
            .expect("months have a last day");
        let expenses =
            stats::expenses_by_category(pool, start, end, None, CategoryLevel::TopLevel).await?;
        for expense in expenses {
            if let Some(category_id) = expense.category_id {
                history
                    .entry(category_id)
                    .or_default()
                    .push(expense.expense_cents);
            }
        }
    }
    let dismissed: HashSet<i64> = sqlx::query_scalar!(
        "SELECT category_id FROM unusual_expense_dismissal WHERE month = ?1",
        month
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    let months = i64::from(HISTORY_MONTHS);
    let mut alerts: Vec<(i64, Alert)> = current
        .into_iter()
        .filter_map(|expense| {
            let category_id = expense.category_id?;
            let past = history.get(&category_id)?;
            let total: i64 = past.iter().sum();
            let spent_months = past.iter().filter(|cents| **cents > 0).count();
            // Compared with `total / months` without rounding.
            let excess = expense.expense_cents * months - total;
            let unusual = spent_months >= MIN_SPENT_MONTHS
                && expense.expense_cents * 100 * months > total * (100 + EXCESS_PERCENT)
                && excess >= MIN_EXCESS_CENTS * months
                && !dismissed.contains(&category_id);
            unusual.then(|| {
                let alert = Alert::UnusualExpense {
                    category_id,
                    month,
                    expense_cents: expense.expense_cents,
                    average_cents: (total + months / 2) / months,
                };
                (excess, alert)
            })
        })
        .collect();
    alerts.sort_by_key(|(excess, _)| -excess);
    Ok(alerts.into_iter().map(|(_, alert)| alert).collect())
}

fn first_day_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("the first day of a month exists")
}

/// Stops showing the unusual expense alert of a category for the month of `month`.
pub async fn dismiss_unusual_expense(
    pool: &SqlitePool,
    category_id: i64,
    month: NaiveDate,
) -> AppResult<()> {
    let exists = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM category WHERE id = ?1"#,
        category_id
    )
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        return Err(CategoryError::NotFound(category_id).into());
    }
    let month = first_day_of_month(month);
    sqlx::query!(
        "INSERT OR IGNORE INTO unusual_expense_dismissal (category_id, month) VALUES (?1, ?2)",
        category_id,
        month
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Stops showing an occurrence as missing.
pub async fn dismiss_missing_occurrence(
    pool: &SqlitePool,
    scheduled_operation_id: i64,
    occurrence_date: NaiveDate,
) -> AppResult<()> {
    matching::check_occurrence(pool, scheduled_operation_id, occurrence_date).await?;
    sqlx::query!(
        "INSERT OR IGNORE INTO missing_occurrence_dismissal (scheduled_operation_id, occurrence_date)
         VALUES (?1, ?2)",
        scheduled_operation_id,
        occurrence_date
    )
    .execute(pool)
    .await?;
    Ok(())
}
