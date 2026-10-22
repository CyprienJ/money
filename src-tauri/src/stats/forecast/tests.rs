use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::schedule::{Frequency, ScheduledOperationInput};
use crate::operation::{self, NewOperation, category};
use crate::stats::StatsError;

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

async fn new_account(pool: &SqlitePool, account_type: AccountType, opening: i64) -> i64 {
    account::create(
        pool,
        NewAccount {
            name: format!("{account_type:?}"),
            account_type,
            institution: None,
            opening_balance_cents: opening,
            opening_date: date("2026-01-01"),
        },
    )
    .await
    .unwrap()
    .id
}

async fn enter(pool: &SqlitePool, account_id: i64, day: &str, amount_cents: i64) -> i64 {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents,
            label: format!("Op {day} {amount_cents}"),
            category_id: None,
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
    .id
}

async fn monthly(
    pool: &SqlitePool,
    account_id: i64,
    label: &str,
    day: i64,
    amount_cents: i64,
    category_id: Option<i64>,
) -> i64 {
    schedule::create(
        pool,
        ScheduledOperationInput {
            account_id,
            label: label.to_owned(),
            amount_cents,
            category_id,
            frequency: Frequency::Monthly,
            day_of_period: day,
            start_date: date("2026-01-01"),
            end_date: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn transfer_category(pool: &SqlitePool) -> i64 {
    category::list(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.is_internal_transfer)
        .unwrap()
        .id
}

fn balance_on(forecast: &BalanceForecast, day: &str) -> i64 {
    forecast
        .points
        .iter()
        .find(|p| p.date == date(day))
        .unwrap()
        .balance_cents
}

#[test]
fn adds_the_occurrences_to_the_current_balance_day_by_day() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking, 100_000).await;
        enter(&pool, checking, "2026-10-02", -20_000).await;
        // Operations after today are not in the current balance.
        enter(&pool, checking, "2026-10-25", -1_000).await;
        monthly(&pool, checking, "Loyer", 25, -50_000, None).await;
        monthly(&pool, checking, "Salaire", 28, 200_000, None).await;

        let forecast = balance_forecast(&pool, date("2026-10-20"), date("2026-11-30"), None)
            .await
            .unwrap();
        assert_eq!(forecast.current_balance_cents, 80_000);
        assert_eq!(forecast.points.len(), 42);
        assert_eq!(forecast.points[0].date, date("2026-10-20"));
        assert_eq!(balance_on(&forecast, "2026-10-24"), 80_000);
        assert_eq!(balance_on(&forecast, "2026-10-25"), 30_000);
        assert_eq!(balance_on(&forecast, "2026-10-28"), 230_000);
        assert_eq!(balance_on(&forecast, "2026-11-30"), 380_000);
        assert_eq!(forecast.lowest.date, date("2026-10-25"));
        assert_eq!(forecast.lowest.balance_cents, 30_000);
        let labels: Vec<(&str, NaiveDate)> = forecast
            .occurrences
            .iter()
            .map(|o| (o.label.as_str(), o.date))
            .collect();
        assert_eq!(
            labels,
            [
                ("Loyer", date("2026-10-25")),
                ("Salaire", date("2026-10-28")),
                ("Loyer", date("2026-11-25")),
                ("Salaire", date("2026-11-28")),
            ]
        );
    });
}

#[test]
fn counts_late_occurrences_today_and_skips_realized_or_missed_ones() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking, 100_000).await;
        let late = monthly(&pool, checking, "Électricité", 17, -6_000, None).await;
        let realized = monthly(&pool, checking, "Internet", 22, -3_000, None).await;
        // Missed: older than the matching window.
        monthly(&pool, checking, "Assurance", 10, -2_000, None).await;
        let paid = enter(&pool, checking, "2026-10-19", -3_000).await;
        matching::match_occurrence(&pool, realized, date("2026-10-22"), paid)
            .await
            .unwrap();

        let forecast = balance_forecast(&pool, date("2026-10-20"), date("2026-10-31"), None)
            .await
            .unwrap();
        assert_eq!(forecast.current_balance_cents, 97_000);
        let counted: Vec<(i64, NaiveDate, NaiveDate)> = forecast
            .occurrences
            .iter()
            .map(|o| (o.scheduled_operation_id, o.occurrence_date, o.date))
            .collect();
        assert_eq!(counted, [(late, date("2026-10-17"), date("2026-10-20"))]);
        assert_eq!(forecast.points[0].balance_cents, 91_000);
        assert_eq!(forecast.lowest.date, date("2026-10-20"));
    });
}

#[test]
fn leaves_internal_transfer_schedules_out_of_several_accounts() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking, 100_000).await;
        let savings = new_account(&pool, AccountType::Savings, 50_000).await;
        let transfer = transfer_category(&pool).await;
        monthly(&pool, checking, "Épargne", 25, -30_000, Some(transfer)).await;
        monthly(&pool, checking, "Loyer", 26, -50_000, None).await;
        let from = date("2026-10-20");
        let to = date("2026-10-31");

        let all = balance_forecast(&pool, from, to, None).await.unwrap();
        assert_eq!(all.current_balance_cents, 150_000);
        assert_eq!(all.occurrences.len(), 1);
        assert_eq!(all.points.last().unwrap().balance_cents, 100_000);

        let both = balance_forecast(&pool, from, to, Some(&[checking, savings]))
            .await
            .unwrap();
        assert_eq!(both.points.last().unwrap().balance_cents, 100_000);

        let alone = balance_forecast(&pool, from, to, Some(&[checking]))
            .await
            .unwrap();
        assert_eq!(alone.occurrences.len(), 2);
        assert_eq!(alone.points.last().unwrap().balance_cents, 20_000);
    });
}

#[test]
fn covers_only_the_accounts_of_the_balance() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking, 100_000).await;
        let archived = new_account(&pool, AccountType::Checking, 10_000).await;
        monthly(&pool, archived, "Ancien", 25, -5_000, None).await;
        account::set_archived(&pool, archived, true).await.unwrap();
        let other = new_account(&pool, AccountType::Savings, 0).await;
        monthly(&pool, other, "Intérêts", 25, 100, None).await;
        let from = date("2026-10-20");
        let to = date("2026-10-31");

        let all = balance_forecast(&pool, from, to, None).await.unwrap();
        assert_eq!(all.current_balance_cents, 100_000);
        assert_eq!(all.occurrences.len(), 1);

        let one = balance_forecast(&pool, from, to, Some(&[checking]))
            .await
            .unwrap();
        assert!(one.occurrences.is_empty());
        assert_eq!(one.lowest.balance_cents, 100_000);
        // The first day of the lowest balance.
        assert_eq!(one.lowest.date, from);
    });
}

#[test]
fn rejects_a_horizon_before_today() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        assert!(matches!(
            balance_forecast(&pool, date("2026-10-20"), date("2026-10-19"), None).await,
            Err(AppError::Stats(StatsError::InvalidPeriod { .. }))
        ));
    });
}
