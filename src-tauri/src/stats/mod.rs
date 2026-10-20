//! Aggregation queries for charts (future AI assistant tools).
//!
//! Every amount comes from `operation_split`, and internal transfers are excluded from income
//! and expenses (but not from balances). An operation is income when its category is an
//! income category, and an expense otherwise, net of refunds; an uncategorized operation is
//! classified by its sign. Expenses are returned as positive amounts.
//!
//! `accounts` restricts a query to these account ids; `None` means every non-archived
//! account with operations (checking, savings, credit card).

use std::collections::BTreeMap;

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::error::{AppResult, ErrorKind};

mod comparison;
mod net_worth;
mod payees;

pub use comparison::{CategoryComparison, DateRange, compare_expenses_by_category};
pub use net_worth::{NetWorth, net_worth_at, net_worth_history};
pub use payees::{LargestExpense, PayeeExpense, largest_expenses, top_payees};

/// How expenses are grouped by category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CategoryLevel {
    /// Subcategories added to their top-level category.
    TopLevel,
    /// Each category as assigned.
    Detailed,
}

/// Length of the periods of `income_vs_expenses`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Granularity {
    Month,
    Year,
}

/// Interval between two points of `balance_history`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BalanceStep {
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryExpense {
    /// `None` for uncategorized expenses.
    pub category_id: Option<i64>,
    /// Net amount spent: positive, or negative when refunds exceed spending.
    pub expense_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PeriodIncomeExpenses {
    /// First day of the month or year.
    pub period_start: NaiveDate,
    pub income_cents: i64,
    /// Positive amount spent, net of refunds.
    pub expense_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BalancePoint {
    pub date: NaiveDate,
    /// Total balance of the accounts at the end of `date`.
    pub balance_cents: i64,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StatsError {
    #[error("the period ends ({to}) before it starts ({from})")]
    InvalidPeriod { from: NaiveDate, to: NaiveDate },
}

impl StatsError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::Validation
    }
}

fn check_period(from: NaiveDate, to: NaiveDate) -> Result<(), StatsError> {
    if to < from {
        return Err(StatsError::InvalidPeriod { from, to });
    }
    Ok(())
}

/// Account filter bound as a JSON array, read with `json_each`; `None` for the default.
fn accounts_json(accounts: Option<&[i64]>) -> Option<String> {
    accounts.map(|ids| {
        let ids: Vec<String> = ids.iter().map(i64::to_string).collect();
        format!("[{}]", ids.join(","))
    })
}

/// Net expenses by category between `from` and `to` (included), the largest first.
pub async fn expenses_by_category(
    pool: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<&[i64]>,
    level: CategoryLevel,
) -> AppResult<Vec<CategoryExpense>> {
    check_period(from, to)?;
    let accounts = accounts_json(accounts);
    let top_level = level == CategoryLevel::TopLevel;
    let rows = sqlx::query_as!(
        CategoryExpense,
        r#"SELECT CASE WHEN ?4 THEN COALESCE(c.parent_id, c.id) ELSE c.id END AS "category_id?: i64",
                  -SUM(s.amount_cents) AS "expense_cents!: i64"
           FROM operation_split s
           JOIN operation o ON o.id = s.operation_id
           JOIN account a ON a.id = o.account_id
           LEFT JOIN category c ON c.id = s.category_id
           WHERE o.date BETWEEN ?1 AND ?2
             AND o.is_internal_transfer = 0
             AND CASE WHEN ?3 IS NULL
                      THEN a.archived = 0
                      ELSE o.account_id IN (SELECT value FROM json_each(?3)) END
             AND NOT COALESCE(c.is_income, s.amount_cents > 0)
           GROUP BY 1
           ORDER BY 2 DESC, 1"#,
        from,
        to,
        accounts,
        top_level
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Income and net expenses for each month or year between `from` and `to` (included),
/// periods without operations included.
pub async fn income_vs_expenses(
    pool: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<&[i64]>,
    granularity: Granularity,
) -> AppResult<Vec<PeriodIncomeExpenses>> {
    check_period(from, to)?;
    let accounts = accounts_json(accounts);
    let rows = sqlx::query!(
        r#"SELECT o.date AS "date!: NaiveDate",
                  SUM(CASE WHEN COALESCE(c.is_income, s.amount_cents > 0)
                           THEN s.amount_cents ELSE 0 END) AS "income_cents!: i64",
                  -SUM(CASE WHEN COALESCE(c.is_income, s.amount_cents > 0)
                            THEN 0 ELSE s.amount_cents END) AS "expense_cents!: i64"
           FROM operation_split s
           JOIN operation o ON o.id = s.operation_id
           JOIN account a ON a.id = o.account_id
           LEFT JOIN category c ON c.id = s.category_id
           WHERE o.date BETWEEN ?1 AND ?2
             AND o.is_internal_transfer = 0
             AND CASE WHEN ?3 IS NULL
                      THEN a.archived = 0
                      ELSE o.account_id IN (SELECT value FROM json_each(?3)) END
           GROUP BY o.date"#,
        from,
        to,
        accounts
    )
    .fetch_all(pool)
    .await?;

    let mut periods: BTreeMap<NaiveDate, PeriodIncomeExpenses> =
        period_starts(from, to, granularity)
            .map(|period_start| {
                let empty = PeriodIncomeExpenses {
                    period_start,
                    income_cents: 0,
                    expense_cents: 0,
                };
                (period_start, empty)
            })
            .collect();
    for row in rows {
        let period = periods
            .get_mut(&period_start(row.date, granularity))
            .expect("every date of the range is in a period");
        period.income_cents += row.income_cents;
        period.expense_cents += row.expense_cents;
    }
    Ok(periods.into_values().collect())
}

fn period_start(date: NaiveDate, granularity: Granularity) -> NaiveDate {
    let month = match granularity {
        Granularity::Month => date.month(),
        Granularity::Year => 1,
    };
    NaiveDate::from_ymd_opt(date.year(), month, 1).expect("first day of a month exists")
}

fn period_starts(
    from: NaiveDate,
    to: NaiveDate,
    granularity: Granularity,
) -> impl Iterator<Item = NaiveDate> {
    let step = match granularity {
        Granularity::Month => Months::new(1),
        Granularity::Year => Months::new(12),
    };
    std::iter::successors(Some(period_start(from, granularity)), move |start| {
        start.checked_add_months(step)
    })
    .take_while(move |start| *start <= to)
}

/// Total balance of the accounts at the end of each step between `from` and `to`: every day,
/// every 7 days from `from`, or at the end of each month; `to` is always the last point.
/// Internal transfers are included. An account counts from its opening date.
pub async fn balance_history(
    pool: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<&[i64]>,
    step: BalanceStep,
) -> AppResult<Vec<BalancePoint>> {
    check_period(from, to)?;
    let accounts = accounts_json(accounts);
    let openings = sqlx::query!(
        r#"SELECT opening_date AS "date!: NaiveDate", opening_balance_cents AS "amount_cents!: i64"
           FROM account
           WHERE type IN ('CHECKING', 'SAVINGS', 'CREDIT_CARD')
             AND CASE WHEN ?1 IS NULL
                      THEN archived = 0
                      ELSE id IN (SELECT value FROM json_each(?1)) END"#,
        accounts
    )
    .fetch_all(pool)
    .await?;
    let movements = sqlx::query!(
        r#"SELECT o.date AS "date!: NaiveDate", SUM(o.amount_cents) AS "amount_cents!: i64"
           FROM operation o
           JOIN account a ON a.id = o.account_id
           WHERE o.date <= ?2
             AND a.type IN ('CHECKING', 'SAVINGS', 'CREDIT_CARD')
             AND CASE WHEN ?1 IS NULL
                      THEN a.archived = 0
                      ELSE o.account_id IN (SELECT value FROM json_each(?1)) END
           GROUP BY o.date"#,
        accounts,
        to
    )
    .fetch_all(pool)
    .await?;

    // Balance changes by date: account openings and operations.
    let mut changes: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    for opening in openings {
        *changes.entry(opening.date).or_default() += opening.amount_cents;
    }
    for movement in movements {
        *changes.entry(movement.date).or_default() += movement.amount_cents;
    }

    let mut changes = changes.into_iter().peekable();
    let mut balance = 0;
    let mut points = Vec::new();
    for date in step_ends(from, to, step) {
        while let Some((_, amount)) = changes.next_if(|(day, _)| *day <= date) {
            balance += amount;
        }
        points.push(BalancePoint {
            date,
            balance_cents: balance,
        });
    }
    Ok(points)
}

/// Dates of the points of `balance_history`, `to` included.
fn step_ends(from: NaiveDate, to: NaiveDate, step: BalanceStep) -> Vec<NaiveDate> {
    let mut dates: Vec<NaiveDate> = match step {
        BalanceStep::Day => from.iter_days().take_while(|d| *d <= to).collect(),
        BalanceStep::Week => {
            std::iter::successors(Some(from), |d| d.checked_add_days(Days::new(7)))
                .take_while(|d| *d <= to)
                .collect()
        }
        BalanceStep::Month => period_starts(from, to, Granularity::Month)
            .filter_map(|start| start.checked_add_months(Months::new(1))?.pred_opt())
            .filter(|end| *end <= to)
            .collect(),
    };
    if dates.last() != Some(&to) {
        dates.push(to);
    }
    dates
}

#[cfg(test)]
mod tests;
