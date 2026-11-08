//! Cash forecast: the balance of today, followed day by day by the occurrences of the schedule
//! that no operation realized yet.
//!
//! An occurrence of the last `matching::DETECTION_WINDOW_DAYS` days without operation is
//! assumed late and counted today; an older one is considered missed. When the forecast covers
//! several accounts, internal transfer schedules are left out: the money stays in the total.
//! Occurrences are converted to euros at the last known rates.

use chrono::{Days, NaiveDate};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{BalancePoint, PersonalBalances, check_period};
use crate::account::{self, AccountSummary};
use crate::currency::Rates;
use crate::error::AppResult;
use crate::operation::schedule::{self, matching};

/// An occurrence counted in the forecast.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ForecastOccurrence {
    pub scheduled_operation_id: i64,
    /// Date of the occurrence in the schedule.
    pub occurrence_date: NaiveDate,
    /// Date it counts in the forecast: today for a late occurrence.
    pub date: NaiveDate,
    pub account_id: i64,
    pub label: String,
    /// In the currency of the account.
    pub amount_cents: i64,
    pub currency: String,
    pub category_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BalanceForecast {
    /// Balance at the end of today from the operations only, in euros like the points.
    pub current_balance_cents: i64,
    /// Projected balance at the end of each day from today to the horizon.
    pub points: Vec<BalancePoint>,
    /// The first day with the lowest projected balance.
    pub lowest: BalancePoint,
    /// Occurrences counted, by date.
    pub occurrences: Vec<ForecastOccurrence>,
}

/// Projected balance of the accounts from `today` to `to` (included). `accounts`: `None` for
/// every non-archived account with operations, as in the other statistics.
pub async fn balance_forecast(
    pool: &SqlitePool,
    today: NaiveDate,
    to: NaiveDate,
    accounts: Option<&[i64]>,
) -> AppResult<BalanceForecast> {
    check_period(today, to)?;
    let included: Vec<AccountSummary> = account::list_summaries(pool, accounts.is_some(), today)
        .await?
        .into_iter()
        .filter(|s| s.account.account_type.has_operations())
        .filter(|s| accounts.is_none_or(|ids| ids.contains(&s.account.id)))
        .collect();
    let several_accounts = included.len() > 1;
    let mut balances = PersonalBalances::default();
    for summary in &included {
        let a = &summary.account;
        if a.opening_date.is_none_or(|opening| opening <= today) {
            balances.add(
                a.id,
                &a.currency,
                a.is_joint,
                summary.balance_cents.unwrap_or(0),
            );
        }
    }
    let rates = Rates::load(pool).await?;
    let current_balance_cents = balances.to_reference(&rates, today);

    let late_from = today
        .checked_sub_days(Days::new(matching::DETECTION_WINDOW_DAYS))
        .unwrap_or(today);
    let mut occurrences: Vec<ForecastOccurrence> =
        schedule::occurrences_between(pool, late_from, to)
            .await?
            .into_iter()
            .filter(|o| o.realized_by.is_none())
            .filter_map(|o| {
                Some(ForecastOccurrence {
                    scheduled_operation_id: o.scheduled_operation_id,
                    occurrence_date: o.date,
                    date: o.date.max(today),
                    account_id: o.account_id,
                    label: o.label,
                    amount_cents: o.amount_cents,
                    currency: included
                        .iter()
                        .find(|s| s.account.id == o.account_id)?
                        .account
                        .currency
                        .clone(),
                    category_id: o.category_id,
                })
            })
            .collect();
    if several_accounts {
        let transfer = internal_transfer_category(pool).await?;
        occurrences.retain(|o| transfer.is_none() || o.category_id != transfer);
    }
    occurrences.sort_by(|a, b| {
        (a.date, a.occurrence_date, &a.label).cmp(&(b.date, b.occurrence_date, &b.label))
    });

    let mut pending = occurrences.iter().peekable();
    let points: Vec<BalancePoint> = today
        .iter_days()
        .take_while(|date| *date <= to)
        .map(|date| {
            while let Some(o) = pending.next_if(|o| o.date <= date) {
                let account = &included
                    .iter()
                    .find(|s| s.account.id == o.account_id)
                    .expect("occurrences belong to included accounts")
                    .account;
                balances.add(
                    account.id,
                    &account.currency,
                    account.is_joint,
                    o.amount_cents,
                );
            }
            BalancePoint {
                date,
                balance_cents: balances.to_reference(&rates, today),
            }
        })
        .collect();
    let lowest = points
        .iter()
        .min_by_key(|point| (point.balance_cents, point.date))
        .cloned()
        .expect("the forecast has at least today");
    Ok(BalanceForecast {
        current_balance_cents,
        points,
        lowest,
        occurrences,
    })
}

async fn internal_transfer_category(pool: &SqlitePool) -> AppResult<Option<i64>> {
    let id = sqlx::query_scalar!(
        r#"SELECT id AS "id!" FROM category WHERE is_internal_transfer = 1 LIMIT 1"#
    )
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

#[cfg(test)]
mod tests;
