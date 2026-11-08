use sqlx::SqlitePool;

use super::*;
use crate::account::NewAccount;
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::schedule::{Frequency, ScheduleError, ScheduledOperationInput};
use crate::operation::{self, NewOperation, category};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

const TODAY: &str = "2026-10-20";

async fn new_account(pool: &SqlitePool, account_type: AccountType, opening: i64) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: format!("{account_type:?}"),
            account_type,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: opening,
            opening_date: Some(date("2026-01-01")),
        },
    )
    .await
    .unwrap()
    .id
}

async fn enter(
    pool: &SqlitePool,
    account_id: i64,
    day: &str,
    amount_cents: i64,
    category_id: Option<i64>,
) -> i64 {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents,
            label: format!("Op {day} {amount_cents}"),
            category_id,
            splits: Vec::new(),
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
    .id
}

async fn monthly(pool: &SqlitePool, account_id: i64, day: i64, amount_cents: i64) -> i64 {
    schedule::create(
        pool,
        ScheduledOperationInput {
            account_id,
            label: "Loyer".to_owned(),
            amount_cents,
            category_id: None,
            frequency: Frequency::Monthly,
            day_of_period: day,
            start_date: date("2026-09-01"),
            end_date: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn top_level(pool: &SqlitePool, name: &str) -> i64 {
    category::list(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.name == name && c.parent_id.is_none())
        .unwrap()
        .id
}

async fn alerts(pool: &SqlitePool) -> Vec<Alert> {
    list(pool, date(TODAY)).await.unwrap()
}

#[test]
fn warns_about_checking_and_savings_accounts_going_negative() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking, 10_000).await;
        monthly(&pool, checking, 25, -50_000).await;
        monthly(&pool, checking, 28, 30_000).await;
        new_account(&pool, AccountType::Savings, 100_000).await;
        // Negative by nature.
        let card = new_account(&pool, AccountType::CreditCard, 0).await;
        enter(&pool, card, "2026-10-10", -20_000, None).await;

        assert_eq!(
            alerts(&pool).await,
            [Alert::NegativeForecast {
                account_id: checking,
                first_negative_date: date("2026-10-25"),
                lowest: BalancePoint {
                    date: date("2026-10-25"),
                    balance_cents: -40_000,
                },
            }]
        );
    });
}

#[test]
fn reports_occurrences_missing_from_an_import_covering_them() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking, 1_000_000).await;
        let rent = monthly(&pool, checking, 5, -80_000).await;
        // Imported up to 2026-10-15: the occurrences of 09-05 and 10-05 are missing.
        enter(&pool, checking, "2026-10-15", -1_000, None).await;
        // Imported up to 2026-10-08 only: the occurrence of 10-05 may still come.
        let late = new_account(&pool, AccountType::Checking, 1_000_000).await;
        let insurance = monthly(&pool, late, 5, -3_000).await;
        let paid = enter(&pool, late, "2026-09-05", -3_000, None).await;
        matching::match_occurrence(&pool, insurance, date("2026-09-05"), paid)
            .await
            .unwrap();
        enter(&pool, late, "2026-10-08", -1_000, None).await;

        let missing = |alerts: Vec<Alert>| -> Vec<(i64, NaiveDate)> {
            alerts
                .into_iter()
                .map(|alert| match alert {
                    Alert::MissingOccurrence { occurrence } => {
                        (occurrence.scheduled_operation_id, occurrence.date)
                    }
                    other => panic!("unexpected {other:?}"),
                })
                .collect()
        };
        assert_eq!(
            missing(alerts(&pool).await),
            [(rent, date("2026-09-05")), (rent, date("2026-10-05"))]
        );

        dismiss_missing_occurrence(&pool, rent, date("2026-09-05"))
            .await
            .unwrap();
        assert_eq!(missing(alerts(&pool).await), [(rent, date("2026-10-05"))]);
        assert!(matches!(
            dismiss_missing_occurrence(&pool, rent, date("2026-10-06")).await,
            Err(AppError::Schedule(ScheduleError::NotAnOccurrence(_)))
        ));
    });
}

#[test]
fn flags_a_category_spent_well_above_its_average() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking, 10_000_000).await;
        let restaurants = top_level(&pool, "Restaurants").await;
        let leisure = top_level(&pool, "Loisirs").await;
        let groceries = top_level(&pool, "Courses").await;
        for month in ["04", "05", "06", "07", "08", "09"] {
            enter(
                &pool,
                checking,
                &format!("2026-{month}-10"),
                -10_000,
                Some(restaurants),
            )
            .await;
            // Above by 100 % but by less than 50 €.
            enter(
                &pool,
                checking,
                &format!("2026-{month}-11"),
                -2_000,
                Some(groceries),
            )
            .await;
        }
        enter(&pool, checking, "2026-10-12", -20_000, Some(restaurants)).await;
        enter(&pool, checking, "2026-10-12", -4_000, Some(groceries)).await;
        // Spent in 2 months of the history only.
        enter(&pool, checking, "2026-08-02", -1_000, Some(leisure)).await;
        enter(&pool, checking, "2026-09-02", -1_000, Some(leisure)).await;
        enter(&pool, checking, "2026-10-02", -50_000, Some(leisure)).await;

        assert_eq!(
            alerts(&pool).await,
            [Alert::UnusualExpense {
                category_id: restaurants,
                month: date("2026-10-01"),
                expense_cents: 20_000,
                average_cents: 10_000,
            }]
        );

        dismiss_unusual_expense(&pool, restaurants, date("2026-10-17"))
            .await
            .unwrap();
        assert!(alerts(&pool).await.is_empty());
        // The dismissal is for October only.
        enter(&pool, checking, "2026-11-05", -30_000, Some(restaurants)).await;
        let november = unusual_expenses(&pool, date("2026-11-30")).await.unwrap();
        assert!(matches!(
            november.as_slice(),
            [Alert::UnusualExpense { category_id, .. }] if *category_id == restaurants
        ));
        assert!(matches!(
            dismiss_unusual_expense(&pool, 999, date("2026-10-01")).await,
            Err(AppError::Category(CategoryError::NotFound(999)))
        ));
    });
}

#[test]
fn stays_quiet_without_reason() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking, 100_000).await;
        monthly(&pool, checking, 25, -50_000).await;
        assert!(alerts(&pool).await.is_empty());
    });
}
