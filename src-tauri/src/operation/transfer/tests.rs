use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::operation::{self, NewOperation, OperationUpdate};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        test(memory_pool().await).await;
    });
}

async fn new_account(pool: &SqlitePool, name: &str) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: name.to_owned(),
            account_type: AccountType::Checking,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: Some(date("2026-01-01")),
        },
    )
    .await
    .unwrap()
    .id
}

async fn enter(pool: &SqlitePool, account_id: i64, day: &str, amount_cents: i64) -> i64 {
    enter_in(pool, account_id, day, amount_cents, None).await
}

async fn enter_in(
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
            label: format!("VIR {amount_cents}"),
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

async fn internal_category(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar!(r#"SELECT id AS "id!" FROM category WHERE is_internal_transfer = 1"#)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn regular_category(pool: &SqlitePool) -> i64 {
    sqlx::query!("INSERT INTO category (name, color) VALUES ('Catégorie de test', 'groceries')")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
}

/// (debit id, credit id) of the proposals.
async fn proposed(pool: &SqlitePool) -> Vec<(i64, i64)> {
    proposals(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|p| (p.debit.operation_id, p.credit.operation_id))
        .collect()
}

#[test]
fn links_a_certain_transfer_as_internal() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let savings = new_account(&pool, "Livret").await;
        let debit = enter(&pool, checking, "2026-03-10", -50_000).await;
        let credit = enter(&pool, savings, "2026-03-13", 50_000).await;

        assert!(proposed(&pool).await.is_empty());
        // Not in this import: nothing linked.
        assert_eq!(link_certain(&pool, Some(999)).await.unwrap(), 0);
        assert_eq!(link_certain(&pool, None).await.unwrap(), 1);

        let internal = internal_category(&pool).await;
        for (id, pair) in [(debit, credit), (credit, debit)] {
            let operation = operation::get(&pool, id).await.unwrap();
            assert_eq!(operation.transfer_pair_id, Some(pair));
            assert_eq!(operation.category_id, Some(internal));
            assert!(operation.is_internal_transfer);
        }
        let other = partner(&pool, debit).await.unwrap().unwrap();
        assert_eq!(
            (
                other.operation_id,
                other.account_name.as_str(),
                other.amount_cents
            ),
            (credit, "Livret", 50_000)
        );
        // Already linked: nothing left to detect.
        assert_eq!(link_certain(&pool, None).await.unwrap(), 0);
    });
}

#[test]
fn proposes_ambiguous_transfers() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let savings = new_account(&pool, "Livret").await;
        let other = new_account(&pool, "Autre").await;
        let debit = enter(&pool, checking, "2026-03-10", -50_000).await;
        let credit = enter(&pool, savings, "2026-03-10", 50_000).await;
        let other_credit = enter(&pool, other, "2026-03-12", 50_000).await;

        assert_eq!(link_certain(&pool, None).await.unwrap(), 0);
        assert_eq!(
            proposed(&pool).await,
            [(debit, credit), (debit, other_credit)]
        );

        // Dismissing one leaves a certain pair.
        dismiss(&pool, debit, other_credit).await.unwrap();
        assert!(proposed(&pool).await.is_empty());
        assert_eq!(link_certain(&pool, None).await.unwrap(), 1);
        assert_eq!(
            operation::get(&pool, debit).await.unwrap().transfer_pair_id,
            Some(credit)
        );
    });
}

#[test]
fn ignores_unlikely_transfers() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let savings = new_account(&pool, "Livret").await;
        let rent = regular_category(&pool).await;
        // Same account.
        enter(&pool, checking, "2026-03-10", -1_000).await;
        enter(&pool, checking, "2026-03-10", 1_000).await;
        // More than 3 days apart.
        enter(&pool, checking, "2026-03-10", -2_000).await;
        enter(&pool, savings, "2026-03-14", 2_000).await;
        // In another category.
        enter_in(&pool, checking, "2026-03-10", -3_000, Some(rent)).await;
        enter(&pool, savings, "2026-03-10", 3_000).await;
        // Not opposite.
        enter(&pool, checking, "2026-03-10", -4_000).await;
        enter(&pool, savings, "2026-03-10", -4_000).await;

        assert_eq!(link_certain(&pool, None).await.unwrap(), 0);
        assert!(proposed(&pool).await.is_empty());
    });
}

#[test]
fn links_by_hand_with_checks() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let savings = new_account(&pool, "Livret").await;
        let rent = regular_category(&pool).await;
        let debit = enter_in(&pool, checking, "2026-03-01", -50_000, Some(rent)).await;
        let far = enter(&pool, savings, "2026-03-20", 50_000).await;
        let close = enter(&pool, savings, "2026-03-04", 50_000).await;
        let same_account = enter(&pool, checking, "2026-03-02", 50_000).await;
        let other_amount = enter(&pool, savings, "2026-03-02", 40_000).await;

        // Any category, up to 15 days apart, the closest first.
        let ids: Vec<i64> = link_candidates(&pool, debit)
            .await
            .unwrap()
            .iter()
            .map(|s| s.operation_id)
            .collect();
        assert_eq!(ids, [close]);

        assert!(matches!(
            link(&pool, debit, same_account).await,
            Err(AppError::Transfer(TransferError::SameAccount))
        ));
        assert!(matches!(
            link(&pool, debit, other_amount).await,
            Err(AppError::Transfer(TransferError::AmountsNotOpposite))
        ));
        link(&pool, debit, far).await.unwrap();
        assert!(
            operation::get(&pool, debit)
                .await
                .unwrap()
                .is_internal_transfer
        );
        assert!(matches!(
            link(&pool, close, debit).await,
            Err(AppError::Transfer(TransferError::AlreadyLinked(id))) if id == debit
        ));
        assert!(matches!(
            link(&pool, debit, 999).await,
            Err(AppError::Operation(OperationError::NotFound(999)))
        ));
    });
}

#[test]
fn unlinks_explicitly_or_when_leaving_the_category() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let savings = new_account(&pool, "Livret").await;
        let debit = enter(&pool, checking, "2026-03-10", -50_000).await;
        let credit = enter(&pool, savings, "2026-03-10", 50_000).await;
        let internal = internal_category(&pool).await;

        link(&pool, debit, credit).await.unwrap();
        unlink(&pool, credit).await.unwrap();
        for id in [debit, credit] {
            let operation = operation::get(&pool, id).await.unwrap();
            assert_eq!(operation.transfer_pair_id, None);
            // The category is kept.
            assert_eq!(operation.category_id, Some(internal));
        }

        link(&pool, debit, credit).await.unwrap();
        let rent = regular_category(&pool).await;
        operation::set_category(&pool, debit, Some(rent))
            .await
            .unwrap();
        assert_eq!(partner(&pool, credit).await.unwrap(), None);
        assert_eq!(
            operation::get(&pool, credit).await.unwrap().category_id,
            Some(internal)
        );

        // Changing another field keeps the link.
        link(&pool, credit, debit).await.unwrap();
        let operation = operation::get(&pool, debit).await.unwrap();
        operation::update(
            &pool,
            debit,
            OperationUpdate {
                date: operation.date,
                value_date: Some(date("2026-03-11")),
                amount_cents: operation.amount_cents,
                label: operation.raw_label.clone(),
                category_id: operation.category_id,
                splits: Vec::new(),
                note: Some("Épargne".to_owned()),
                tags: Vec::new(),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            operation::get(&pool, debit).await.unwrap().transfer_pair_id,
            Some(credit)
        );

        // Deleting one side unlinks the other.
        operation::delete(&pool, debit).await.unwrap();
        assert_eq!(partner(&pool, credit).await.unwrap(), None);
    });
}
