use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, AccountUpdate, NewAccount};
use crate::alert::{self, Alert};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::transfer::{self, TransferError};
use crate::operation::{self, NewOperation, category};
use crate::stats::{self, BalanceStep, CategoryLevel};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

async fn new_account(pool: &SqlitePool, name: &str, currency: &str, opening: i64) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: name.to_owned(),
            account_type: AccountType::Checking,
            currency: currency.to_owned(),
            institution: None,
            opening_balance_cents: opening,
            opening_date: date("2026-01-01"),
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
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
    .id
}

fn update_of(account: &account::Account, currency: &str) -> AccountUpdate {
    AccountUpdate {
        is_joint: false,
        name: account.name.clone(),
        currency: currency.to_owned(),
        institution: account.institution.clone(),
        opening_balance_cents: account.opening_balance_cents,
        opening_date: account.opening_date,
    }
}

#[test]
fn converts_at_the_rate_of_the_day() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        set_manual_rate(&pool, "USD", date("2026-01-10"), 900_000_000)
            .await
            .unwrap();
        set_manual_rate(&pool, "USD", date("2026-02-10"), 800_000_000)
            .await
            .unwrap();
        set_manual_rate(&pool, "GBP", date("2026-01-01"), 1_150_000_000)
            .await
            .unwrap();
        let rates = Rates::load(&pool).await.unwrap();

        // The first rate before any, the last one on or before the day afterwards.
        assert_eq!(rates.rate("USD", date("2026-01-05")), Some(900_000_000));
        assert_eq!(rates.rate("USD", date("2026-02-09")), Some(900_000_000));
        assert_eq!(rates.rate("USD", date("2026-02-10")), Some(800_000_000));
        assert_eq!(rates.rate("EUR", date("2026-02-10")), Some(RATE_SCALE));
        assert_eq!(rates.rate("CHF", date("2026-02-10")), None);

        assert_eq!(rates.to_reference(-1_000, "USD", date("2026-01-20")), -900);
        assert_eq!(rates.to_reference(1_000, "CHF", date("2026-01-20")), 0);
        // 100 $ at 0.80 € ÷ 1.15 € = 69.565… £.
        assert_eq!(
            rates.convert(10_000, "USD", "GBP", date("2026-02-15")),
            Some(6_957)
        );
    });
}

#[test]
fn checks_the_rates_entered_by_hand() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let day = date("2026-10-20");
        assert!(matches!(
            set_manual_rate(&pool, "JPY", day, 6_000_000).await,
            Err(AppError::Currency(CurrencyError::Unsupported(code))) if code == "JPY"
        ));
        assert!(matches!(
            set_manual_rate(&pool, "EUR", day, RATE_SCALE).await,
            Err(AppError::Currency(CurrencyError::ReferenceRate))
        ));
        assert!(matches!(
            set_manual_rate(&pool, "USD", day, 0).await,
            Err(AppError::Currency(CurrencyError::InvalidRate))
        ));

        set_manual_rate(&pool, " usd ", day, 900_000_000)
            .await
            .unwrap();
        set_manual_rate(&pool, "USD", day, 890_000_000)
            .await
            .unwrap();
        let rates = list_rates(&pool, "USD").await.unwrap();
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].rate_nano, 890_000_000);
        delete_rate(&pool, "USD", day).await.unwrap();
        assert!(list_rates(&pool, "USD").await.unwrap().is_empty());
    });
}

#[test]
fn the_currency_of_an_account_is_fixed_once_it_has_data() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let created = account::create(
            &pool,
            NewAccount {
                is_joint: false,
                name: "Compte".to_owned(),
                account_type: AccountType::Checking,
                currency: "XYZ".to_owned(),
                institution: None,
                opening_balance_cents: 0,
                opening_date: date("2026-01-01"),
            },
        )
        .await;
        assert!(matches!(
            created,
            Err(AppError::Currency(CurrencyError::Unsupported(_)))
        ));

        let id = new_account(&pool, "Compte US", "usd", 0).await;
        let current = account::get(&pool, id).await.unwrap();
        assert_eq!(current.currency, "USD");
        let current = account::update(&pool, id, update_of(&current, "GBP"))
            .await
            .unwrap();
        assert_eq!(current.currency, "GBP");

        let operation_id = enter(&pool, id, "2026-01-15", -1_000, None).await;
        let currency: String = sqlx::query_scalar("SELECT currency FROM operation WHERE id = ?1")
            .bind(operation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(currency, "GBP");
        assert!(matches!(
            account::update(&pool, id, update_of(&current, "EUR")).await,
            Err(AppError::Currency(CurrencyError::AccountCurrencyLocked))
        ));
        let mut renamed = update_of(&current, "GBP");
        renamed.name = "Compte UK".to_owned();
        assert_eq!(
            account::update(&pool, id, renamed).await.unwrap().name,
            "Compte UK"
        );
    });
}

#[test]
fn statistics_are_in_euros() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let categories = category::list(&pool).await.unwrap();
        let groceries = categories
            .iter()
            .find(|c| c.name == "Courses" && c.parent_id.is_none())
            .unwrap()
            .id;
        let euros = new_account(&pool, "Courant", "EUR", 100_000).await;
        let dollars = new_account(&pool, "Compte US", "USD", 10_000).await;
        enter(&pool, euros, "2026-01-12", -2_000, Some(groceries)).await;
        enter(&pool, dollars, "2026-01-15", -5_000, Some(groceries)).await;

        // Without any rate, the dollars count 0 and an alert says so.
        let alerts = alert::list(&pool, date("2026-02-28")).await.unwrap();
        assert!(alerts.contains(&Alert::MissingRate {
            currency: "USD".to_owned()
        }));
        let uses = in_use(&pool).await.unwrap();
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].latest, None);

        set_manual_rate(&pool, "USD", date("2026-01-01"), 900_000_000)
            .await
            .unwrap();
        set_manual_rate(&pool, "USD", date("2026-02-01"), 800_000_000)
            .await
            .unwrap();
        let alerts = alert::list(&pool, date("2026-02-28")).await.unwrap();
        assert!(
            !alerts
                .iter()
                .any(|a| matches!(a, Alert::MissingRate { .. }))
        );

        // A flow at the rate of its day: 20 € + 50 $ × 0.90.
        let expenses = stats::expenses_by_category(
            &pool,
            date("2026-01-01"),
            date("2026-02-28"),
            None,
            CategoryLevel::TopLevel,
        )
        .await
        .unwrap();
        assert_eq!(expenses[0].category_id, Some(groceries));
        assert_eq!(expenses[0].expense_cents, 6_500);

        // A balance at the rate of its day: 50 $ × 0.90, then × 0.80.
        let points = stats::balance_history(
            &pool,
            date("2026-01-31"),
            date("2026-02-28"),
            None,
            BalanceStep::Month,
        )
        .await
        .unwrap();
        let balances: Vec<i64> = points.iter().map(|p| p.balance_cents).collect();
        assert_eq!(balances, [98_000 + 4_500, 98_000 + 4_000]);
        let net_worth = stats::net_worth_at(&pool, date("2026-02-28"))
            .await
            .unwrap();
        assert_eq!(net_worth.cash_cents, 102_000);

        let summaries = account::list_summaries(&pool, false, date("2026-02-28"))
            .await
            .unwrap();
        let us = summaries.iter().find(|s| s.account.id == dollars).unwrap();
        assert_eq!(us.balance_cents, Some(5_000));
        assert_eq!(us.reference_balance_cents, Some(4_000));
    });
}

#[test]
fn transfers_link_accounts_of_the_same_currency_only() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let euros = new_account(&pool, "Courant", "EUR", 0).await;
        let dollars = new_account(&pool, "Compte US", "USD", 0).await;
        let debit = enter(&pool, euros, "2026-01-12", -10_000, None).await;
        let credit = enter(&pool, dollars, "2026-01-12", 10_000, None).await;

        assert!(
            transfer::link_candidates(&pool, debit)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            transfer::link(&pool, debit, credit).await,
            Err(AppError::Transfer(TransferError::DifferentCurrencies))
        ));
    });
}
