use chrono::NaiveDate;
use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::labeling::renormalize_all;
use crate::operation::{self, NewOperation, Operation};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

async fn checking_account(pool: &SqlitePool) -> i64 {
    account::create(
        pool,
        NewAccount {
            name: "Courant".to_owned(),
            account_type: AccountType::Checking,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
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
    label: &str,
    cents: i64,
) -> Operation {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents: cents,
            label: label.to_owned(),
            category_id: None,
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
}

fn names(payees: &[Payee]) -> Vec<(&str, Vec<&str>)> {
    payees
        .iter()
        .map(|p| {
            (
                p.name.as_str(),
                p.labels.iter().map(String::as_str).collect(),
            )
        })
        .collect()
}

#[test]
fn default_names_are_readable_labels() {
    assert_eq!(default_name("CARREFOUR MKT"), "Carrefour Mkt");
    assert_eq!(default_name("PHARMACIE DU CENTRE"), "Pharmacie du Centre");
    assert_eq!(default_name("LE PETIT BISTROT"), "Le Petit Bistrot");
    assert_eq!(
        default_name("LOYER SCI DES ÉRABLES"),
        "Loyer Sci des Érables"
    );
    assert_eq!(
        default_name("BOULANGERIE SAINT-MARTIN"),
        "Boulangerie Saint-Martin"
    );
    assert_eq!(default_name("CAFE L'ARBRE"), "Cafe L'Arbre");
    assert_eq!(default_name("RETRAIT DAB PARIS 11"), "Retrait Dab Paris 11");
    assert_eq!(default_name("FNAC.COM"), "Fnac.com");
    assert_eq!(default_name(""), "Sans libellé");
}

#[test]
fn every_label_gets_a_payee() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = checking_account(&pool).await;
        let first = enter(
            &pool,
            account_id,
            "2026-10-01",
            "CB CARREFOUR MKT 01/10",
            -1_000,
        )
        .await;
        let second = enter(
            &pool,
            account_id,
            "2026-10-02",
            "CB CARREFOUR MKT 02/10",
            -500,
        )
        .await;
        let other = enter(
            &pool,
            account_id,
            "2026-10-03",
            "PRLV SEPA FREE MOBILE",
            -1_999,
        )
        .await;

        assert_eq!(first.payee_name, "Carrefour Mkt");
        assert_eq!(first.payee_id, second.payee_id);
        assert_ne!(first.payee_id, other.payee_id);

        let payees = list(&pool).await.unwrap();
        assert_eq!(
            names(&payees),
            vec![
                ("Carrefour Mkt", vec!["CARREFOUR MKT"]),
                ("Free Mobile", vec!["FREE MOBILE"]),
            ]
        );
        assert_eq!(payees[0].operation_count, 2);
        assert_eq!(payees[0].total_cents, Some(-1_500));
        assert_eq!(payees[0].last_date, Some(date("2026-10-02")));
    });
}

#[test]
fn renames_and_refuses_a_taken_name() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = checking_account(&pool).await;
        let carrefour = enter(&pool, account_id, "2026-10-01", "CB CARREFOUR MKT", -1_000).await;
        let free = enter(&pool, account_id, "2026-10-02", "FREE MOBILE", -1_999).await;
        let carrefour_id = carrefour.payee_id.unwrap();

        let renamed = rename(&pool, carrefour_id, "  Carrefour ").await.unwrap();
        assert_eq!(renamed.name, "Carrefour");
        let operation = operation::get(&pool, carrefour.id).await.unwrap();
        assert_eq!(operation.payee_name, "Carrefour");

        // Same name as another payee, ignoring case: merging is offered instead.
        let taken = rename(&pool, carrefour_id, "free mobile")
            .await
            .unwrap_err();
        assert!(matches!(
            taken,
            AppError::Payee(PayeeError::NameTaken { ref name, payee_id })
                if name == "Free Mobile" && Some(payee_id) == free.payee_id
        ));
        // Changing the case of its own name is fine.
        rename(&pool, carrefour_id, "CARREFOUR").await.unwrap();
        assert!(matches!(
            rename(&pool, carrefour_id, " ").await.unwrap_err(),
            AppError::Payee(PayeeError::EmptyName)
        ));
        assert!(matches!(
            rename(&pool, 999, "X").await.unwrap_err(),
            AppError::Payee(PayeeError::NotFound(999))
        ));
    });
}

#[test]
fn merges_payees_and_detaches_labels() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = checking_account(&pool).await;
        let market = enter(&pool, account_id, "2026-10-01", "CB CARREFOUR MKT", -1_000).await;
        let city = enter(&pool, account_id, "2026-10-02", "CB CARREFOUR CITY", -500).await;
        let express = enter(
            &pool,
            account_id,
            "2026-10-03",
            "CB CARREFOUR EXPRESS",
            -200,
        )
        .await;
        let target = market.payee_id.unwrap();
        let sources = [city.payee_id.unwrap(), express.payee_id.unwrap()];

        assert!(matches!(
            merge(&pool, target, &[target]).await.unwrap_err(),
            AppError::Payee(PayeeError::MergeIntoItself)
        ));
        rename(&pool, target, "Carrefour").await.unwrap();
        let merged = merge(&pool, target, &sources).await.unwrap();
        assert_eq!(
            merged.labels,
            ["CARREFOUR CITY", "CARREFOUR EXPRESS", "CARREFOUR MKT"]
        );
        assert_eq!(merged.operation_count, 3);
        assert_eq!(list(&pool).await.unwrap().len(), 1);
        assert_eq!(
            operation::get(&pool, city.id).await.unwrap().payee_name,
            "Carrefour"
        );

        // A new operation of a merged label joins the merged payee.
        let later = enter(&pool, account_id, "2026-10-04", "CB CARREFOUR CITY", -300).await;
        assert_eq!(later.payee_id, Some(target));

        let detached = detach(&pool, "CARREFOUR CITY").await.unwrap();
        assert_eq!(detached.name, "Carrefour City");
        assert_eq!(detached.operation_count, 2);
        assert_eq!(
            operation::get(&pool, later.id).await.unwrap().payee_id,
            Some(detached.id)
        );
        assert!(matches!(
            detach(&pool, "CARREFOUR CITY").await.unwrap_err(),
            AppError::Payee(PayeeError::LastLabel)
        ));
        assert!(matches!(
            detach(&pool, "INCONNU").await.unwrap_err(),
            AppError::Payee(PayeeError::LabelNotFound(_))
        ));

        // The default name of a detached label may be taken: a number is added.
        rename(&pool, target, "Carrefour Express").await.unwrap();
        let detached = detach(&pool, "CARREFOUR EXPRESS").await.unwrap();
        assert_eq!(detached.name, "Carrefour Express (2)");
    });
}

#[test]
fn a_new_label_joins_the_payee_named_like_it() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = checking_account(&pool).await;
        let market = enter(&pool, account_id, "2026-10-01", "CB CARREFOUR MKT", -1_000).await;
        rename(&pool, market.payee_id.unwrap(), "Carrefour")
            .await
            .unwrap();

        let plain = enter(&pool, account_id, "2026-10-02", "CB CARREFOUR", -500).await;
        assert_eq!(plain.payee_id, market.payee_id);
    });
}

#[test]
fn existing_operations_get_payees_and_keep_them_when_renormalized() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = checking_account(&pool).await;
        // Written before payees existed, and with labels normalized by older rules.
        for (raw, normalized) in [
            ("CB BOULANGERIE PAUL 12/10", "BOULANGERIE PAUL 12/10"),
            ("CB BOULANGERIE PAUL 13/10", "BOULANGERIE PAUL"),
            ("CB FNAC 14/10", "FNAC"),
        ] {
            sqlx::query!(
                "INSERT INTO operation
                 (account_id, date, amount_cents, raw_label, normalized_label,
                  created_at, updated_at)
                 VALUES (?1, '2026-10-12', -250, ?2, ?3,
                         '2026-10-19T10:00:00Z', '2026-10-19T10:00:00Z')",
                account_id,
                raw,
                normalized
            )
            .execute(&pool)
            .await
            .unwrap();
        }

        assert_eq!(assign_missing(&pool).await.unwrap(), 3);
        assert_eq!(assign_missing(&pool).await.unwrap(), 0);
        assert_eq!(list(&pool).await.unwrap().len(), 3);

        // The stale label becomes "BOULANGERIE PAUL", which already has its payee: the
        // operation joins it, and the stale payee, left without operations, is no longer listed.
        assert_eq!(renormalize_all(&pool).await.unwrap(), 1);
        let payees = list(&pool).await.unwrap();
        assert_eq!(
            names(&payees),
            vec![
                ("Boulangerie Paul", vec!["BOULANGERIE PAUL"]),
                ("Fnac", vec!["FNAC"]),
            ]
        );
        assert_eq!(payees[0].operation_count, 2);
    });
}

#[test]
fn renormalized_operations_keep_their_payee() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = checking_account(&pool).await;
        let operation = enter(&pool, account_id, "2026-10-01", "CB FNAC 01/10", -1_000).await;
        rename(&pool, operation.payee_id.unwrap(), "Fnac Darty")
            .await
            .unwrap();
        // As if the normalization rules had changed since the operation was recorded.
        sqlx::query!(
            "UPDATE operation SET normalized_label = 'FNAC 01/10' WHERE id = ?1",
            operation.id
        )
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(renormalize_all(&pool).await.unwrap(), 1);
        let operation = operation::get(&pool, operation.id).await.unwrap();
        assert_eq!(operation.normalized_label, "FNAC");
        assert_eq!(operation.payee_name, "Fnac Darty");
    });
}

#[test]
fn internal_transfers_stay_out_of_payee_totals() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = checking_account(&pool).await;
        let transfer = operation::category::list(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.is_internal_transfer)
            .unwrap()
            .id;
        let saving = enter(
            &pool,
            account_id,
            "2026-10-01",
            "VIR VERS LIVRET A",
            -30_000,
        )
        .await;
        operation::set_category(&pool, saving.id, Some(transfer))
            .await
            .unwrap();

        let payees = list(&pool).await.unwrap();
        assert_eq!(payees[0].operation_count, 1);
        assert_eq!(payees[0].total_cents, None);
    });
}
