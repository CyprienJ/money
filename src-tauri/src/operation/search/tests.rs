use chrono::NaiveDate;
use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::operation::category::{self, CategoryColor, CategoryInput};
use crate::operation::{self, NewOperation};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        test(memory_pool().await).await;
    });
}

/// Ids of the categories and accounts of the sample data.
struct Sample {
    checking: i64,
    savings: i64,
    archived: i64,
    groceries: i64,
    supermarket: i64,
    leisure: i64,
}

async fn new_account(pool: &SqlitePool, name: &str, opening_cents: i64) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: name.to_owned(),
            account_type: AccountType::Checking,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: opening_cents,
            opening_date: date("2026-01-01"),
        },
    )
    .await
    .unwrap()
    .id
}

async fn new_category(pool: &SqlitePool, name: &str, parent_id: Option<i64>) -> i64 {
    category::create(
        pool,
        CategoryInput {
            name: name.to_owned(),
            parent_id,
            color: CategoryColor::Groceries,
            is_income: false,
        },
    )
    .await
    .unwrap()
    .id
}

#[allow(clippy::too_many_arguments)]
async fn enter(
    pool: &SqlitePool,
    account_id: i64,
    day: &str,
    label: &str,
    amount_cents: i64,
    category_id: Option<i64>,
    tags: &[&str],
) {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents,
            label: label.to_owned(),
            category_id,
            splits: Vec::new(),
            note: None,
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        },
    )
    .await
    .unwrap();
}

async fn sample(pool: &SqlitePool) -> Sample {
    let checking = new_account(pool, "Courant", 100_000).await;
    let savings = new_account(pool, "Livret", 500_000).await;
    let archived = new_account(pool, "Ancien", 0).await;
    let groceries = new_category(pool, "Alimentation test", None).await;
    let supermarket = new_category(pool, "Supermarché test", Some(groceries)).await;
    let leisure = new_category(pool, "Sorties test", None).await;

    enter(
        pool,
        checking,
        "2026-03-02",
        "Carrefour",
        -6_428,
        Some(supermarket),
        &[],
    )
    .await;
    enter(
        pool,
        checking,
        "2026-03-10",
        "Marché",
        -1_250,
        Some(groceries),
        &["Vacances"],
    )
    .await;
    enter(
        pool,
        checking,
        "2026-04-05",
        "Cinéma",
        -2_400,
        Some(leisure),
        &["Vacances"],
    )
    .await;
    enter(pool, checking, "2026-04-20", "Salaire", 250_000, None, &[]).await;
    enter(
        pool,
        savings,
        "2026-03-15",
        "Carrefour",
        -15_000,
        None,
        &["Vacances"],
    )
    .await;
    enter(pool, archived, "2026-03-20", "Carrefour", -3_000, None, &[]).await;
    account::set_archived(pool, archived, true).await.unwrap();

    Sample {
        checking,
        savings,
        archived,
        groceries,
        supermarket,
        leisure,
    }
}

async fn labels(pool: &SqlitePool, filter: OperationFilter) -> Vec<(String, i64)> {
    search(pool, &filter)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.operation.raw_label, row.operation.amount_cents))
        .collect()
}

fn pairs(expected: &[(&str, i64)]) -> Vec<(String, i64)> {
    expected
        .iter()
        .map(|(label, cents)| ((*label).to_owned(), *cents))
        .collect()
}

#[test]
fn searches_every_unarchived_account_with_their_own_balances() {
    run(|pool| async move {
        let s = sample(&pool).await;
        let rows = search(&pool, &OperationFilter::default()).await.unwrap();
        let found: Vec<(i64, &str, i64)> = rows
            .iter()
            .map(|r| {
                (
                    r.operation.account_id,
                    r.operation.raw_label.as_str(),
                    r.balance_cents,
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                (s.checking, "Salaire", 339_922),
                (s.checking, "Cinéma", 89_922),
                (s.savings, "Carrefour", 485_000),
                (s.checking, "Marché", 92_322),
                (s.checking, "Carrefour", 93_572),
            ]
        );

        let with_archived = OperationFilter {
            include_archived: true,
            ..OperationFilter::default()
        };
        assert_eq!(search(&pool, &with_archived).await.unwrap().len(), 6);
        // An archived account searched on its own.
        let archived_only = OperationFilter {
            account_id: Some(s.archived),
            ..OperationFilter::default()
        };
        assert_eq!(
            labels(&pool, archived_only).await,
            pairs(&[("Carrefour", -3_000)])
        );
    });
}

#[test]
fn filters_on_the_period_and_the_amount() {
    run(|pool| async move {
        let s = sample(&pool).await;
        let april = OperationFilter {
            account_id: Some(s.checking),
            from: Some(date("2026-04-01")),
            to: Some(date("2026-04-20")),
            ..OperationFilter::default()
        };
        assert_eq!(
            labels(&pool, april).await,
            pairs(&[("Salaire", 250_000), ("Cinéma", -2_400)])
        );

        // Expenses between 20 and 150 €, bounds included.
        let expenses = OperationFilter {
            direction: Some(Direction::Debit),
            min_cents: Some(2_400),
            max_cents: Some(15_000),
            ..OperationFilter::default()
        };
        assert_eq!(
            labels(&pool, expenses).await,
            pairs(&[
                ("Cinéma", -2_400),
                ("Carrefour", -15_000),
                ("Carrefour", -6_428)
            ])
        );

        let credits = OperationFilter {
            direction: Some(Direction::Credit),
            ..OperationFilter::default()
        };
        assert_eq!(labels(&pool, credits).await, pairs(&[("Salaire", 250_000)]));
    });
}

#[test]
fn filters_on_the_category_with_its_subcategories() {
    run(|pool| async move {
        let s = sample(&pool).await;
        let category = |id| OperationFilter {
            category: Some(CategoryFilter::Category { id }),
            ..OperationFilter::default()
        };
        assert_eq!(
            labels(&pool, category(s.groceries)).await,
            pairs(&[("Marché", -1_250), ("Carrefour", -6_428)])
        );
        assert_eq!(
            labels(&pool, category(s.supermarket)).await,
            pairs(&[("Carrefour", -6_428)])
        );
        let uncategorized = OperationFilter {
            category: Some(CategoryFilter::Uncategorized),
            ..OperationFilter::default()
        };
        assert_eq!(
            labels(&pool, uncategorized).await,
            pairs(&[("Salaire", 250_000), ("Carrefour", -15_000)])
        );
    });
}

#[test]
fn combines_the_payee_tag_and_account_filters() {
    run(|pool| async move {
        let s = sample(&pool).await;
        let rows = search(&pool, &OperationFilter::default()).await.unwrap();
        let carrefour = rows
            .iter()
            .find(|r| r.operation.raw_label == "Carrefour")
            .and_then(|r| r.operation.payee_id);
        let vacances = crate::tag::list(&pool).await.unwrap()[0].id;

        let payee = OperationFilter {
            payee_id: carrefour,
            ..OperationFilter::default()
        };
        assert_eq!(
            labels(&pool, payee).await,
            pairs(&[("Carrefour", -15_000), ("Carrefour", -6_428)])
        );

        let tagged = OperationFilter {
            tag_id: Some(vacances),
            ..OperationFilter::default()
        };
        assert_eq!(
            labels(&pool, tagged).await,
            pairs(&[
                ("Cinéma", -2_400),
                ("Carrefour", -15_000),
                ("Marché", -1_250)
            ])
        );

        // Every filter at once.
        let combined = OperationFilter {
            account_id: Some(s.checking),
            tag_id: Some(vacances),
            category: Some(CategoryFilter::Category { id: s.leisure }),
            direction: Some(Direction::Debit),
            ..OperationFilter::default()
        };
        let rows = search(&pool, &combined).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].operation.raw_label, "Cinéma");
        assert_eq!(rows[0].operation.tags, ["Vacances"]);
        assert_eq!(rows[0].balance_cents, 89_922);
    });
}
