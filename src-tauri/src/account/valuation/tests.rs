use super::*;
use crate::account::{self, AccountType, NewAccount, list_summaries};
use crate::db::tests::memory_pool;

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async { test(memory_pool().await).await });
}

async fn new_account(pool: &SqlitePool, name: &str, account_type: AccountType) -> i64 {
    account::create(
        pool,
        NewAccount {
            name: name.to_owned(),
            account_type,
            institution: None,
            opening_balance_cents: 0,
            opening_date: date("2026-01-01"),
        },
    )
    .await
    .unwrap()
    .id
}

fn input(day: &str, value_cents: i64) -> ValuationInput {
    ValuationInput {
        date: date(day),
        value_cents,
    }
}

fn account_error<T: std::fmt::Debug>(result: AppResult<T>) -> AccountError {
    match result {
        Err(AppError::Account(error)) => error,
        other => panic!("expected an account error, got {other:?}"),
    }
}

#[test]
fn records_and_lists_valuations_most_recent_first() {
    run(|pool| async move {
        let pea = new_account(&pool, "PEA", AccountType::Investment).await;
        let june = create(&pool, pea, input("2026-06-30", 4_000_000))
            .await
            .unwrap();
        let september = create(&pool, pea, input("2026-09-30", 4_182_000))
            .await
            .unwrap();
        assert_eq!(september.account_id, pea);
        assert_eq!(
            list(&pool, pea).await.unwrap(),
            vec![september.clone(), june.clone()]
        );

        let corrected = update(&pool, june.id, input("2026-06-29", 3_990_000))
            .await
            .unwrap();
        assert_eq!(
            (corrected.date, corrected.value_cents),
            (date("2026-06-29"), 3_990_000)
        );
        delete(&pool, june.id).await.unwrap();
        assert_eq!(list(&pool, pea).await.unwrap(), vec![september]);
        assert!(matches!(
            account_error(delete(&pool, june.id).await),
            AccountError::ValuationNotFound(_)
        ));
    });
}

#[test]
fn rejects_invalid_valuations() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant", AccountType::Checking).await;
        let loan = new_account(&pool, "Prêt", AccountType::Liability).await;

        assert!(matches!(
            account_error(create(&pool, checking, input("2026-06-30", 100)).await),
            AccountError::NotValued
        ));
        assert!(matches!(
            account_error(create(&pool, loan, input("2026-06-30", -100)).await),
            AccountError::NegativeValuation
        ));
        assert!(matches!(
            account_error(create(&pool, loan, input("2025-12-31", 100)).await),
            AccountError::ValuationBeforeOpening(_)
        ));
        create(&pool, loan, input("2026-06-30", 100)).await.unwrap();
        let other = create(&pool, loan, input("2026-07-31", 90)).await.unwrap();
        assert!(matches!(
            account_error(create(&pool, loan, input("2026-06-30", 95)).await),
            AccountError::DuplicateValuationDate(_)
        ));
        assert!(matches!(
            account_error(update(&pool, other.id, input("2026-06-30", 95)).await),
            AccountError::DuplicateValuationDate(_)
        ));
        assert!(matches!(
            account_error(list(&pool, 999).await),
            AccountError::NotFound(999)
        ));
    });
}

#[test]
fn summaries_use_the_last_valuation_at_the_date() {
    run(|pool| async move {
        let pea = new_account(&pool, "PEA", AccountType::Investment).await;
        let flat = new_account(&pool, "Appartement", AccountType::Asset).await;
        let loan = new_account(&pool, "Prêt", AccountType::Liability).await;
        create(&pool, pea, input("2026-06-30", 4_000_000))
            .await
            .unwrap();
        create(&pool, pea, input("2026-09-30", 4_182_000))
            .await
            .unwrap();
        create(&pool, flat, input("2026-01-15", 21_000_000))
            .await
            .unwrap();
        create(&pool, loan, input("2026-09-05", 12_699_000))
            .await
            .unwrap();

        let at = |day: &'static str| {
            let pool = pool.clone();
            async move {
                list_summaries(&pool, false, date(day))
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|s| (s.account.name, s.balance_cents, s.valued_on))
                    .collect::<Vec<_>>()
            }
        };
        let names_order = at("2026-10-20").await;
        let find = |rows: &[(String, Option<i64>, Option<NaiveDate>)], name: &str| {
            rows.iter().find(|r| r.0 == name).cloned().unwrap()
        };
        assert_eq!(
            find(&names_order, "PEA"),
            ("PEA".to_owned(), Some(4_182_000), Some(date("2026-09-30")))
        );
        assert_eq!(find(&names_order, "Appartement").1, Some(21_000_000));
        // A debt counts negatively.
        assert_eq!(
            find(&names_order, "Prêt"),
            (
                "Prêt".to_owned(),
                Some(-12_699_000),
                Some(date("2026-09-05"))
            )
        );

        let august = at("2026-08-31").await;
        assert_eq!(find(&august, "PEA").1, Some(4_000_000));
        // No valuation yet.
        assert_eq!(find(&august, "Prêt").1, None);
        assert_eq!(find(&august, "Prêt").2, None);
    });
}
