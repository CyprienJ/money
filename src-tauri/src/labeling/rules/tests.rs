use chrono::NaiveDate;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::{self, NewOperation, category};

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        test(pool).await;
    });
}

async fn category_id(pool: &SqlitePool, name: &str) -> i64 {
    category::list(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap()
        .id
}

async fn new_operation(pool: &SqlitePool, label: &str, category_id: Option<i64>) -> i64 {
    let account_id = match account::list(pool, false).await.unwrap().first() {
        Some(account) => account.id,
        None => {
            account::create(
                pool,
                NewAccount {
                    name: "Courant".to_owned(),
                    account_type: AccountType::Checking,
                    institution: None,
                    opening_balance_cents: 0,
                    opening_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                },
            )
            .await
            .unwrap()
            .id
        }
    };
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            value_date: None,
            amount_cents: 120,
            label: label.to_owned(),
            category_id,
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
    .id
}

fn input(pattern: &str, category_id: i64) -> RuleInput {
    RuleInput {
        pattern: pattern.to_owned(),
        category_id,
    }
}

#[test]
fn matches_ignoring_case_accents_and_spaces() {
    assert_eq!(fold("Intérêts  nets"), "INTERETS NETS");
    assert_eq!(fold("Œuvre Ça"), "OEUVRE CA");
}

#[test]
fn the_first_rule_in_order_wins() {
    run(|pool| async move {
        let income = category_id(&pool, "Intérêts").await;
        let other = category_id(&pool, "Divers").await;
        let first = create(&pool, input("  interets   nets ", income))
            .await
            .unwrap();
        let second = create(&pool, input("versés", other)).await.unwrap();
        assert_eq!(first.pattern, "interets nets");

        let mut conn = pool.acquire().await.unwrap();
        let rules = Rules::load(&mut conn).await.unwrap();
        let label = "INTÉRÊTS NETS VERSÉS POUR COMPTE D'ÉPARGNE";
        assert_eq!(rules.find(label), Some((income, "interets nets")));
        assert_eq!(rules.find("CARREFOUR"), None);
        drop(conn);

        reorder(&pool, &[second.id, first.id]).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let rules = Rules::load(&mut conn).await.unwrap();
        assert_eq!(rules.find(label), Some((other, "versés")));
        drop(conn);

        let listed: Vec<i64> = list(&pool).await.unwrap().iter().map(|r| r.id).collect();
        assert_eq!(listed, vec![second.id, first.id]);
        assert!(matches!(
            reorder(&pool, &[first.id]).await,
            Err(AppError::Rule(RuleError::InvalidOrder))
        ));
    });
}

#[test]
fn applies_a_rule_to_operations_to_categorize() {
    run(|pool| async move {
        let income = category_id(&pool, "Intérêts").await;
        let groceries = category_id(&pool, "Courses").await;
        let april = new_operation(&pool, "Intérêts nets versés le Apr 5, 2025", None).await;
        new_operation(&pool, "Intérêts nets versés le May 5, 2025", None).await;
        let categorized = new_operation(&pool, "Intérêts nets versés", Some(groceries)).await;
        new_operation(&pool, "CB CARREFOUR", None).await;

        assert_eq!(count_pending(&pool, "interets").await.unwrap(), 2);
        let rule = create(&pool, input("Intérêts nets", income)).await.unwrap();
        assert_eq!(rule.pending_count, 2);

        assert_eq!(apply(&pool, rule.id).await.unwrap(), 2);
        assert_eq!(
            operation::get(&pool, april).await.unwrap().category_id,
            Some(income)
        );
        // Categorized operations are left as they are.
        assert_eq!(
            operation::get(&pool, categorized)
                .await
                .unwrap()
                .category_id,
            Some(groceries)
        );
        assert_eq!(list(&pool).await.unwrap()[0].pending_count, 0);
        assert_eq!(apply(&pool, rule.id).await.unwrap(), 0);
    });
}

#[test]
fn applying_an_internal_transfer_rule_sets_the_flag() {
    run(|pool| async move {
        let transfer = category_id(&pool, "Virement interne").await;
        let id = new_operation(&pool, "VIR VERS LIVRET A", None).await;
        let rule = create(&pool, input("livret a", transfer)).await.unwrap();
        apply(&pool, rule.id).await.unwrap();
        assert!(
            operation::get(&pool, id)
                .await
                .unwrap()
                .is_internal_transfer
        );
    });
}

#[test]
fn updates_deletes_and_validates() {
    run(|pool| async move {
        let income = category_id(&pool, "Intérêts").await;
        let groceries = category_id(&pool, "Courses").await;
        let rule = create(&pool, input("A", income)).await.unwrap();

        let updated = update(&pool, rule.id, input("B", groceries)).await.unwrap();
        assert_eq!(
            (updated.pattern.as_str(), updated.category_id),
            ("B", groceries)
        );
        assert!(matches!(
            create(&pool, input("  ", income)).await,
            Err(AppError::Rule(RuleError::EmptyPattern))
        ));
        assert!(matches!(
            create(&pool, input("C", 9999)).await,
            Err(AppError::Rule(RuleError::CategoryNotFound(9999)))
        ));

        delete(&pool, rule.id).await.unwrap();
        assert!(matches!(
            delete(&pool, rule.id).await,
            Err(AppError::Rule(RuleError::NotFound(_)))
        ));
        assert!(matches!(
            update(&pool, rule.id, input("B", income)).await,
            Err(AppError::Rule(RuleError::NotFound(_)))
        ));
    });
}

#[test]
fn rules_follow_a_deleted_category() {
    run(|pool| async move {
        let groceries = category_id(&pool, "Courses").await;
        let target = category_id(&pool, "Divers").await;
        let created = category::create(
            &pool,
            category::CategoryInput {
                name: "Temporaire".to_owned(),
                parent_id: None,
                color: category::CategoryColor::Neutral,
                is_income: false,
            },
        )
        .await
        .unwrap();
        create(&pool, input("A", created.id)).await.unwrap();
        create(&pool, input("B", groceries)).await.unwrap();

        // Moved to the category taking the operations.
        category::delete(&pool, created.id, Some(target))
            .await
            .unwrap();
        let rules = list(&pool).await.unwrap();
        assert_eq!(rules[0].category_id, target);

        // Deleted with a category whose operations become uncategorized.
        let other = category::create(
            &pool,
            category::CategoryInput {
                name: "Autre temporaire".to_owned(),
                parent_id: None,
                color: category::CategoryColor::Neutral,
                is_income: false,
            },
        )
        .await
        .unwrap();
        update(&pool, rules[0].id, input("A", other.id))
            .await
            .unwrap();
        category::delete(&pool, other.id, None).await.unwrap();
        assert_eq!(list(&pool).await.unwrap().len(), 1);
    });
}
