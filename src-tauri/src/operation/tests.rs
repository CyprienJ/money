use chrono::NaiveDate;
use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountError, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::{AppError, AppResult};
use search::CategoryFilter;

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        test(pool).await;
    });
}

async fn new_account(pool: &SqlitePool, account_type: AccountType, opening_cents: i64) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: format!("{account_type:?}"),
            account_type,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: opening_cents,
            opening_date: Some(date("2026-10-01")),
        },
    )
    .await
    .unwrap()
    .id
}

fn manual(account_id: i64, day: &str, amount_cents: i64, label: &str) -> NewOperation {
    NewOperation {
        account_id,
        date: date(day),
        value_date: None,
        amount_cents,
        label: label.to_owned(),
        category_id: None,
        splits: Vec::new(),
        note: None,
        tags: Vec::new(),
    }
}

async fn internal_transfer_category(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar!("SELECT id FROM category WHERE is_internal_transfer = 1")
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

/// Inserts an operation as an import would, and returns its id.
async fn imported(pool: &SqlitePool, account_id: i64) -> i64 {
    let batch_id = sqlx::query!(
        "INSERT INTO import_batch
         (account_id, source_type, source_file_name, source_file_hash, imported_at, balance_check_status)
         VALUES (?1, 'CSV', 'releve.csv', 'hash', '2026-10-19T10:00:00Z', 'OK')",
        account_id
    )
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid();
    let id = sqlx::query!(
        "INSERT INTO operation
         (account_id, date, amount_cents, raw_label, normalized_label, import_batch_id,
          created_at, updated_at)
         VALUES (?1, '2026-10-19', -6428, 'CB CARREFOUR MKT 18/10', 'CARREFOUR MKT', ?2,
                 '2026-10-19T10:00:00Z', '2026-10-19T10:00:00Z')",
        account_id,
        batch_id
    )
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid();
    sqlx::query!(
        "INSERT INTO operation_split (operation_id, category_id, amount_cents) VALUES (?1, NULL, -6428)",
        id
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

fn unchanged(operation: &Operation) -> OperationUpdate {
    OperationUpdate {
        date: operation.date,
        value_date: operation.value_date,
        amount_cents: operation.amount_cents,
        label: operation.raw_label.clone(),
        category_id: operation.category_id,
        splits: operation.splits.clone(),
        note: operation.note.clone(),
        tags: operation.tags.clone(),
    }
}

async fn split_count(pool: &SqlitePool, operation_id: i64) -> i64 {
    sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM operation_split WHERE operation_id = ?1"#,
        operation_id
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[test]
fn creates_a_manual_operation_with_its_split() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let category_id = regular_category(&pool).await;

        let created = create(
            &pool,
            NewOperation {
                category_id: Some(category_id),
                splits: Vec::new(),
                ..manual(account_id, "2026-10-08", -840, "  Boulangerie   Martin ")
            },
        )
        .await
        .unwrap();

        assert_eq!(created.raw_label, "Boulangerie   Martin");
        assert_eq!(created.normalized_label, "BOULANGERIE MARTIN");
        assert_eq!(created.amount_cents, -840);
        assert_eq!(created.category_id, Some(category_id));
        assert_eq!(created.import_batch_id, None);
        assert!(!created.is_internal_transfer);
        assert_eq!(split_count(&pool, created.id).await, 1);
        assert_eq!(get(&pool, created.id).await.unwrap(), created);
    });
}

#[test]
fn derives_the_internal_transfer_flag_from_the_category() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let transfer = internal_transfer_category(&pool).await;

        let created = create(
            &pool,
            NewOperation {
                category_id: Some(transfer),
                splits: Vec::new(),
                ..manual(account_id, "2026-10-15", -30000, "Virement vers Livret A")
            },
        )
        .await
        .unwrap();
        assert!(created.is_internal_transfer);

        let cleared = set_category(&pool, created.id, None).await.unwrap();
        assert!(!cleared.is_internal_transfer);
        assert_eq!(cleared.category_id, None);
    });
}

#[test]
fn rejects_invalid_manual_operations() {
    run(|pool| async move {
        let checking = new_account(&pool, AccountType::Checking, 0).await;
        let investment = new_account(&pool, AccountType::Investment, 0).await;

        let error = |result: AppResult<Operation>| result.unwrap_err();
        assert!(matches!(
            error(create(&pool, manual(checking, "2026-10-08", -840, "  ")).await),
            AppError::Operation(OperationError::EmptyLabel)
        ));
        assert!(matches!(
            error(create(&pool, manual(checking, "2026-10-08", 0, "X")).await),
            AppError::Operation(OperationError::ZeroAmount)
        ));
        assert!(matches!(
            error(create(&pool, manual(investment, "2026-10-08", -840, "X")).await),
            AppError::Operation(OperationError::AccountWithoutOperations)
        ));
        assert!(matches!(
            error(create(&pool, manual(checking, "2026-09-30", -840, "X")).await),
            AppError::Operation(OperationError::DateBeforeOpening(_))
        ));
        assert!(matches!(
            error(create(&pool, manual(99, "2026-10-08", -840, "X")).await),
            AppError::Account(AccountError::NotFound(99))
        ));
        let unknown_category = NewOperation {
            category_id: Some(99),
            splits: Vec::new(),
            ..manual(checking, "2026-10-08", -840, "X")
        };
        assert!(matches!(
            error(create(&pool, unknown_category).await),
            AppError::Operation(OperationError::CategoryNotFound(99))
        ));

        let count: i64 = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!: i64" FROM operation"#)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    });
}

#[test]
fn lists_most_recent_first_with_running_balance() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 41323).await;
        let other = new_account(&pool, AccountType::Savings, 0).await;
        for (day, amount, label) in [
            ("2026-10-05", -84200, "Crédit immobilier"),
            ("2026-10-01", 328000, "Salaire"),
            ("2026-10-05", -9715, "Carrefour Market"),
            ("2026-10-19", -6428, "Carrefour Market"),
        ] {
            create(&pool, manual(account_id, day, amount, label))
                .await
                .unwrap();
        }
        create(&pool, manual(other, "2026-10-15", 30000, "Virement"))
            .await
            .unwrap();

        let rows = list(&pool, account_id, None, None).await.unwrap();
        let summary: Vec<(&str, i64)> = rows
            .iter()
            .map(|r| (r.operation.raw_label.as_str(), r.balance_cents))
            .collect();
        // Same-day operations are ordered by creation: Crédit immobilier, then Carrefour.
        assert_eq!(
            summary,
            [
                ("Carrefour Market", 268980),
                ("Carrefour Market", 275408),
                ("Crédit immobilier", 285123),
                ("Salaire", 369323),
            ]
        );

        let october_5: Vec<i64> = list(
            &pool,
            account_id,
            Some(date("2026-10-02")),
            Some(date("2026-10-05")),
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.balance_cents)
        .collect();
        assert_eq!(october_5, [275408, 285123]);
    });
}

#[test]
fn updates_every_field_of_a_manual_operation() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 1000).await;
        let category_id = regular_category(&pool).await;
        let created = create(&pool, manual(account_id, "2026-10-08", -840, "Boulangerie"))
            .await
            .unwrap();

        let updated = update(
            &pool,
            created.id,
            OperationUpdate {
                date: date("2026-10-09"),
                value_date: Some(date("2026-10-10")),
                amount_cents: -920,
                label: "Boulangerie Martin".to_owned(),
                category_id: Some(category_id),
                splits: Vec::new(),
                note: None,
                tags: Vec::new(),
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.date, date("2026-10-09"));
        assert_eq!(updated.value_date, Some(date("2026-10-10")));
        assert_eq!(updated.amount_cents, -920);
        assert_eq!(updated.raw_label, "Boulangerie Martin");
        assert_eq!(updated.normalized_label, "BOULANGERIE MARTIN");
        assert_eq!(updated.category_id, Some(category_id));

        let split_amount: i64 = sqlx::query_scalar!(
            "SELECT amount_cents FROM operation_split WHERE operation_id = ?1",
            created.id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(split_amount, -920);
        assert_eq!(
            list(&pool, account_id, None, None).await.unwrap()[0].balance_cents,
            80
        );
    });
}

#[test]
fn locks_the_bank_fields_of_an_imported_operation() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let category_id = regular_category(&pool).await;
        let id = imported(&pool, account_id).await;
        let operation = get(&pool, id).await.unwrap();

        for changes in [
            OperationUpdate {
                amount_cents: -6000,
                ..unchanged(&operation)
            },
            OperationUpdate {
                date: date("2026-10-18"),
                ..unchanged(&operation)
            },
            OperationUpdate {
                label: "Carrefour".to_owned(),
                ..unchanged(&operation)
            },
        ] {
            assert!(matches!(
                update(&pool, id, changes).await,
                Err(AppError::Operation(OperationError::ImportedFieldsLocked))
            ));
        }

        let updated = update(
            &pool,
            id,
            OperationUpdate {
                value_date: Some(date("2026-10-20")),
                category_id: Some(category_id),
                splits: Vec::new(),
                ..unchanged(&operation)
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.value_date, Some(date("2026-10-20")));
        assert_eq!(updated.category_id, Some(category_id));
        assert_eq!(updated.raw_label, "CB CARREFOUR MKT 18/10");
        assert_eq!(updated.normalized_label, "CARREFOUR MKT");
    });
}

#[test]
fn deletes_manual_operations_only() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let manual_id = create(&pool, manual(account_id, "2026-10-08", -840, "Boulangerie"))
            .await
            .unwrap()
            .id;
        let imported_id = imported(&pool, account_id).await;

        delete(&pool, manual_id).await.unwrap();
        assert!(matches!(
            get(&pool, manual_id).await,
            Err(AppError::Operation(OperationError::NotFound(_)))
        ));
        assert_eq!(split_count(&pool, manual_id).await, 0);

        assert!(matches!(
            delete(&pool, imported_id).await,
            Err(AppError::Operation(
                OperationError::ImportedOperationNotDeletable
            ))
        ));
        assert!(get(&pool, imported_id).await.is_ok());
    });
}

#[test]
fn reports_unknown_operations() {
    run(|pool| async move {
        assert!(matches!(
            set_category(&pool, 5, None).await,
            Err(AppError::Operation(OperationError::NotFound(5)))
        ));
        assert!(matches!(
            delete(&pool, 5).await,
            Err(AppError::Operation(OperationError::NotFound(5)))
        ));
    });
}

#[test]
fn records_the_note_and_tags_of_a_manual_operation() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let operation = create(
            &pool,
            NewOperation {
                note: Some("  Anniversaire de Léa  ".to_owned()),
                tags: vec![
                    "vacances".to_owned(),
                    " Cadeaux ".to_owned(),
                    "VACANCES".to_owned(),
                    "  ".to_owned(),
                ],
                ..manual(account_id, "2026-10-08", -4500, "Fnac")
            },
        )
        .await
        .unwrap();
        assert_eq!(operation.note.as_deref(), Some("Anniversaire de Léa"));
        // Trimmed, repeats ignoring case and empty names skipped, by name.
        assert_eq!(operation.tags, ["Cadeaux", "vacances"]);

        let updated = update(
            &pool,
            operation.id,
            OperationUpdate {
                note: Some(" ".to_owned()),
                tags: vec!["Vacances".to_owned(), "Rembourser".to_owned()],
                ..unchanged(&operation)
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.note, None);
        // An existing tag keeps its name.
        assert_eq!(updated.tags, ["Rembourser", "vacances"]);
        let rows = list(&pool, account_id, None, None).await.unwrap();
        assert_eq!(rows[0].operation, updated);
    });
}

#[test]
fn annotates_an_imported_operation() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let id = imported(&pool, account_id).await;
        let operation = get(&pool, id).await.unwrap();
        assert_eq!((operation.note.as_deref(), operation.tags.len()), (None, 0));

        let updated = update(
            &pool,
            id,
            OperationUpdate {
                note: Some("Courses de la semaine".to_owned()),
                tags: vec!["Maison".to_owned()],
                ..unchanged(&operation)
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.note.as_deref(), Some("Courses de la semaine"));
        assert_eq!(updated.tags, ["Maison"]);
        assert_eq!(updated.raw_label, operation.raw_label);
    });
}

#[test]
fn editing_the_note_keeps_the_remembered_category() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let id = imported(&pool, account_id).await;
        let old_category = regular_category(&pool).await;
        let operation = set_category(&pool, id, Some(old_category)).await.unwrap();
        let new_category = internal_transfer_category(&pool).await;
        crate::labeling::memory::remember(
            &mut pool.acquire().await.unwrap(),
            "CARREFOUR MKT",
            Some(new_category),
            "2026-10-20T00:00:00Z",
        )
        .await
        .unwrap();

        let changes = OperationUpdate {
            note: Some("Courses".to_owned()),
            ..unchanged(&operation)
        };
        update(&pool, id, changes).await.unwrap();
        let remembered: i64 = sqlx::query_scalar(
            "SELECT category_id FROM label_category_memory WHERE normalized_label = 'CARREFOUR MKT'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(remembered, new_category);
    });
}

async fn category_named(pool: &SqlitePool, name: &str) -> i64 {
    category::list(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap()
        .id
}

fn parts(parts: &[(i64, i64)]) -> Vec<SplitLine> {
    parts
        .iter()
        .map(|&(category_id, amount_cents)| SplitLine {
            category_id,
            amount_cents,
        })
        .collect()
}

#[test]
fn splits_an_operation_over_several_categories() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 10_000).await;
        let food = category_named(&pool, "Alimentation").await;
        let housing = category_named(&pool, "Loyer ou crédit").await;
        let split = create(
            &pool,
            NewOperation {
                splits: parts(&[(food, -3_000), (housing, -1_000)]),
                ..manual(account_id, "2026-10-10", -4_000, "CB HYPERMARCHE")
            },
        )
        .await
        .unwrap();
        assert_eq!(split.category_id, None);
        assert_eq!(split.splits, parts(&[(food, -3_000), (housing, -1_000)]));
        assert!(!split.is_internal_transfer);
        create(&pool, manual(account_id, "2026-10-11", -500, "CB BOULANGERIE"))
            .await
            .unwrap();

        // One row per operation, the running balance counting it once.
        let rows = list(&pool, account_id, None, None).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].operation.id, split.id);
        assert_eq!(rows[1].operation.splits, split.splits);
        assert_eq!(rows[1].balance_cents, 6_000);
        assert_eq!(rows[0].balance_cents, 5_500);

        // Found by the category of any part; never "to categorize".
        let search_category = |category: CategoryFilter| {
            let pool = pool.clone();
            async move {
                let filter = search::OperationFilter {
                    account_id: Some(account_id),
                    category: Some(category),
                    ..Default::default()
                };
                search::search(&pool, &filter)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|row| row.operation.id)
                    .collect::<Vec<_>>()
            }
        };
        assert_eq!(
            search_category(CategoryFilter::Category { id: housing }).await,
            [split.id]
        );
        assert_eq!(search_category(CategoryFilter::Uncategorized).await.len(), 1);
        assert!(
            !search_category(CategoryFilter::Uncategorized)
                .await
                .contains(&split.id)
        );

        // The split is not remembered for the label.
        let remembered = sqlx::query_scalar!(
            "SELECT category_id FROM label_category_memory WHERE normalized_label = ?1",
            split.normalized_label
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(remembered, None);
    });
}

#[test]
fn rejects_invalid_splits() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let food = category_named(&pool, "Alimentation").await;
        let housing = category_named(&pool, "Loyer ou crédit").await;
        let transfer = internal_transfer_category(&pool).await;
        let cases = [
            (parts(&[(food, -4_000)]), "too few"),
            (parts(&[(food, -3_000), (transfer, -1_000)]), "transfer"),
            (parts(&[(food, -5_000), (housing, 1_000)]), "sign"),
            (parts(&[(food, -4_000), (housing, 0)]), "zero"),
            (parts(&[(food, -3_000), (housing, -500)]), "total"),
        ];
        for (splits, case) in cases {
            let result = create(
                &pool,
                NewOperation {
                    splits,
                    ..manual(account_id, "2026-10-10", -4_000, "CB HYPERMARCHE")
                },
            )
            .await;
            let expected = match case {
                "too few" => matches!(
                    result,
                    Err(AppError::Operation(OperationError::SplitTooFewParts))
                ),
                "transfer" => matches!(
                    result,
                    Err(AppError::Operation(OperationError::SplitInternalTransfer))
                ),
                "sign" | "zero" => matches!(
                    result,
                    Err(AppError::Operation(OperationError::SplitPartAmount))
                ),
                _ => matches!(
                    result,
                    Err(AppError::Operation(OperationError::SplitTotalMismatch {
                        expected: -4_000,
                        actual: -3_500
                    }))
                ),
            };
            assert!(expected, "{case}: {result:?}");
        }
        assert!(list(&pool, account_id, None, None).await.unwrap().is_empty());
    });
}

#[test]
fn edits_and_removes_the_split_of_an_operation() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let food = category_named(&pool, "Alimentation").await;
        let housing = category_named(&pool, "Loyer ou crédit").await;

        // An imported operation can be split, its amount staying locked.
        let id = imported(&pool, account_id).await;
        let operation = get(&pool, id).await.unwrap();
        let split = update(
            &pool,
            id,
            OperationUpdate {
                splits: parts(&[(food, -6_000), (housing, -428)]),
                ..unchanged(&operation)
            },
        )
        .await
        .unwrap();
        assert_eq!(split.splits.len(), 2);
        assert_eq!(split_count(&pool, id).await, 2);

        // Changing the amount of a manual split operation needs parts that follow.
        let manual_split = create(
            &pool,
            NewOperation {
                splits: parts(&[(food, -3_000), (housing, -1_000)]),
                ..manual(account_id, "2026-10-10", -4_000, "CB HYPERMARCHE")
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            update(
                &pool,
                manual_split.id,
                OperationUpdate {
                    amount_cents: -5_000,
                    ..unchanged(&manual_split)
                },
            )
            .await,
            Err(AppError::Operation(OperationError::SplitTotalMismatch { .. }))
        ));

        // Giving a single category, from the form or the table, removes the split.
        let single = update(
            &pool,
            manual_split.id,
            OperationUpdate {
                category_id: Some(food),
                splits: Vec::new(),
                amount_cents: -5_000,
                ..unchanged(&manual_split)
            },
        )
        .await
        .unwrap();
        assert_eq!((single.category_id, single.splits.len()), (Some(food), 0));
        assert_eq!(split_count(&pool, manual_split.id).await, 1);
        let collapsed = set_category(&pool, id, Some(housing)).await.unwrap();
        assert_eq!((collapsed.category_id, collapsed.splits.len()), (Some(housing), 0));
        let amount = sqlx::query_scalar!(
            "SELECT amount_cents FROM operation_split WHERE operation_id = ?1",
            id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(amount, -6_428);
    });
}

#[test]
fn keeps_the_parts_of_split_operations_categorized() {
    run(|pool| async move {
        let account_id = new_account(&pool, AccountType::Checking, 0).await;
        let food = category_named(&pool, "Alimentation").await;
        let housing = category_named(&pool, "Loyer ou crédit").await;
        let extra = regular_category(&pool).await;
        let split = create(
            &pool,
            NewOperation {
                splits: parts(&[(food, -3_000), (extra, -1_000)]),
                ..manual(account_id, "2026-10-10", -4_000, "CB HYPERMARCHE")
            },
        )
        .await
        .unwrap();

        assert!(matches!(
            category::delete(&pool, extra, None).await,
            Err(AppError::Category(category::CategoryError::UsedInSplits))
        ));
        let transfer = internal_transfer_category(&pool).await;
        assert!(matches!(
            category::delete(&pool, extra, Some(transfer)).await,
            Err(AppError::Category(category::CategoryError::UsedInSplits))
        ));
        category::delete(&pool, extra, Some(housing)).await.unwrap();
        let moved = get(&pool, split.id).await.unwrap();
        assert_eq!(moved.splits, parts(&[(food, -3_000), (housing, -1_000)]));
    });
}
