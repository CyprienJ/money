use chrono::NaiveDate;
use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::{self, NewOperation, Operation};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        test(pool).await;
    });
}

async fn tagged(pool: &SqlitePool, label: &str, tags: &[&str]) -> Operation {
    let account_id = account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: format!("Compte {label}"),
            account_type: AccountType::Checking,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: Some(date("2026-01-01")),
        },
    )
    .await
    .unwrap()
    .id;
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date("2026-10-08"),
            value_date: None,
            amount_cents: -1200,
            label: label.to_owned(),
            category_id: None,
            splits: Vec::new(),
            note: None,
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        },
    )
    .await
    .unwrap()
}

fn counts(tags: &[Tag]) -> Vec<(&str, i64)> {
    tags.iter()
        .map(|t| (t.name.as_str(), t.operation_count))
        .collect()
}

#[test]
fn cleans_tag_names() {
    assert_eq!(
        clean_name("  Week-end   à Lyon "),
        Some("Week-end à Lyon".to_owned())
    );
    assert_eq!(clean_name(" \t "), None);
}

#[test]
fn lists_tags_by_name_with_their_operation_count() {
    run(|pool| async move {
        tagged(&pool, "Fnac", &["vacances", "Cadeaux"]).await;
        tagged(&pool, "Sncf", &["Vacances"]).await;
        assert_eq!(
            counts(&list(&pool).await.unwrap()),
            [("Cadeaux", 1), ("vacances", 2)]
        );
    });
}

#[test]
fn renames_a_tag_unless_the_name_is_taken() {
    run(|pool| async move {
        let operation = tagged(&pool, "Fnac", &["vacances", "Cadeaux"]).await;
        let tags = list(&pool).await.unwrap();
        let (cadeaux, vacances) = (tags[0].id, tags[1].id);

        // Changing the case only is allowed.
        let renamed = rename(&pool, vacances, " Vacances  d'été ").await.unwrap();
        assert_eq!(renamed.name, "Vacances d'été");
        assert_eq!(
            operation::get(&pool, operation.id).await.unwrap().tags,
            ["Cadeaux", "Vacances d'été"]
        );

        assert!(matches!(
            rename(&pool, vacances, "CADEAUX").await,
            Err(AppError::Tag(TagError::NameTaken(name))) if name == "Cadeaux"
        ));
        assert!(matches!(
            rename(&pool, cadeaux, " ").await,
            Err(AppError::Tag(TagError::EmptyName))
        ));
        assert!(matches!(
            rename(&pool, 99, "Autre").await,
            Err(AppError::Tag(TagError::NotFound(99)))
        ));
    });
}

#[test]
fn deletes_a_tag_from_its_operations() {
    run(|pool| async move {
        let operation = tagged(&pool, "Fnac", &["vacances", "Cadeaux"]).await;
        let cadeaux = list(&pool).await.unwrap()[0].id;
        delete(&pool, cadeaux).await.unwrap();
        assert_eq!(
            operation::get(&pool, operation.id).await.unwrap().tags,
            ["vacances"]
        );
        assert!(matches!(
            delete(&pool, cadeaux).await,
            Err(AppError::Tag(TagError::NotFound(_)))
        ));
    });
}

#[test]
fn keeps_a_tag_when_its_operations_are_deleted() {
    run(|pool| async move {
        let operation = tagged(&pool, "Fnac", &["vacances"]).await;
        operation::delete(&pool, operation.id).await.unwrap();
        assert_eq!(counts(&list(&pool).await.unwrap()), [("vacances", 0)]);
    });
}
