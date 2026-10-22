use std::sync::atomic::{AtomicU32, Ordering};

use chrono::NaiveDate;
use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::{self, NewOperation, OperationError};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

/// An uncategorized line to import.
fn line(day: &str, amount_cents: i64, label: &str) -> ImportLine {
    ImportLine {
        line: RawLine {
            date: date(day),
            value_date: None,
            label: label.to_owned(),
            amount_cents,
            note: None,
            source_category: None,
            source_page: None,
            source_line: None,
        },
        category_id: None,
        chosen: false,
    }
}

/// Fresh directory under the system temporary directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let name = format!(
            "money-batch-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&path);
        TempDir(path)
    }

    fn files(&self) -> Vec<PathBuf> {
        match std::fs::read_dir(&self.0) {
            Ok(entries) => entries.map(|entry| entry.unwrap().path()).collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool, TempDir) -> F) {
    tauri::async_runtime::block_on(async {
        test(memory_pool().await, TempDir::new()).await;
    });
}

/// Like `run`, with a data file on disk: `VACUUM INTO` from an in-memory database writes
/// its copy in memory too.
fn run_on_file<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool, TempDir) -> F) {
    tauri::async_runtime::block_on(async {
        let dir = TempDir::new();
        std::fs::create_dir_all(&dir.0).unwrap();
        let pool = crate::db::open(&dir.0.join("data.sqlite")).await.unwrap();
        test(pool.clone(), dir).await;
        pool.close().await;
    });
}

async fn new_account(pool: &SqlitePool, account_type: AccountType) -> i64 {
    account::create(
        pool,
        NewAccount {
            name: format!("{account_type:?}"),
            account_type,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: date("2026-09-01"),
        },
    )
    .await
    .unwrap()
    .id
}

fn csv_batch(account_id: i64, hash: &str) -> NewImportBatch {
    NewImportBatch {
        account_id,
        source_type: SourceType::Csv,
        source_file_name: "releve.csv".to_owned(),
        source_file_hash: hash.to_owned(),
        statement_start_balance_cents: None,
        statement_end_balance_cents: None,
        balance_check_status: BalanceCheckStatus::NotAvailable,
        stored_file: None,
    }
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    // `table` is always a literal table name from these tests.
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[test]
fn hashes_files_with_sha256() {
    assert_eq!(
        file_hash(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn imports_lines_as_uncategorized_operations() {
    run(|pool, backups| async move {
        let account_id = new_account(&pool, AccountType::Checking).await;
        let lines = [
            line("2026-09-02", -420, "  CB  Boulangerie "),
            ImportLine {
                line: RawLine {
                    value_date: Some(date("2026-09-04")),
                    ..line("2026-09-03", 245_000, "VIR SALAIRE").line
                },
                category_id: None,
                chosen: false,
            },
        ];
        let batch = import(&pool, &backups.0, csv_batch(account_id, "h1"), &lines)
            .await
            .unwrap();
        assert_eq!(batch.account_id, account_id);
        assert_eq!(batch.source_type, SourceType::Csv);
        assert_eq!(batch.source_file_hash, "h1");
        assert_eq!(batch.balance_check_status, BalanceCheckStatus::NotAvailable);
        assert_eq!(batch.operation_count, 2);

        let rows = operation::list(&pool, account_id, None, None)
            .await
            .unwrap();
        let operations: Vec<_> = rows.iter().rev().map(|row| &row.operation).collect();
        assert_eq!(operations[0].raw_label, "CB  Boulangerie");
        assert_eq!(operations[0].normalized_label, "BOULANGERIE");
        assert_eq!(operations[0].amount_cents, -420);
        assert_eq!(operations[1].value_date, Some(date("2026-09-04")));
        assert!(operations.iter().all(|operation| {
            operation.import_batch_id == Some(batch.id) && operation.category_id.is_none()
        }));
        assert_eq!(count(&pool, "operation_split").await, 2);
    });
}

#[test]
fn backs_up_the_data_before_inserting() {
    run_on_file(|pool, dir| async move {
        let backups = TempDir(dir.0.join("backups"));
        let account_id = new_account(&pool, AccountType::Checking).await;
        import(
            &pool,
            &backups.0,
            csv_batch(account_id, "h1"),
            &[line("2026-09-02", -420, "A")],
        )
        .await
        .unwrap();

        let files = backups.files();
        assert_eq!(files.len(), 1);
        let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("money-") && name.ends_with(".sqlite"),
            "{name}"
        );

        let copy = SqlitePool::connect(&format!("sqlite:{}", files[0].display()))
            .await
            .unwrap();
        assert_eq!(count(&copy, "account").await, 1);
        assert_eq!(count(&copy, "import_batch").await, 0);
        copy.close().await;
    });
}

#[test]
fn backup_names_do_not_collide() {
    run_on_file(|pool, dir| async move {
        let backups = TempDir(dir.0.join("backups"));
        assert_eq!(backup_count(&backups.0).unwrap(), 0);
        let first = backup(&pool, &backups.0).await.unwrap();
        let second = backup(&pool, &backups.0).await.unwrap();
        assert_ne!(first, second);
        assert_eq!(backups.files().len(), 2);
        std::fs::write(backups.0.join("notes.txt"), "not a backup").unwrap();
        assert_eq!(backup_count(&backups.0).unwrap(), 2);
    });
}

#[test]
fn rejects_invalid_imports_without_writing_anything() {
    run_on_file(|pool, dir| async move {
        let backups = TempDir(dir.0.join("backups"));
        let account_id = new_account(&pool, AccountType::Checking).await;
        let investment_id = new_account(&pool, AccountType::Investment).await;
        let batch = || csv_batch(account_id, "h1");

        let result = import(&pool, &backups.0, batch(), &[]).await;
        assert!(matches!(result, Err(AppError::Import(ImportError::NoLine))));

        let lines = [line("2026-09-02", 0, "A")];
        let result = import(&pool, &backups.0, batch(), &lines).await;
        assert!(matches!(
            result,
            Err(AppError::Import(ImportError::InvalidLine))
        ));

        let lines = [line("2026-09-02", -1, " ")];
        let result = import(&pool, &backups.0, batch(), &lines).await;
        assert!(matches!(
            result,
            Err(AppError::Import(ImportError::InvalidLine))
        ));

        let lines = [line("2026-09-02", -1, "A"), line("2026-08-31", -1, "B")];
        let result = import(&pool, &backups.0, batch(), &lines).await;
        assert!(matches!(
            result,
            Err(AppError::Operation(OperationError::DateBeforeOpening(_)))
        ));

        let lines = [line("2026-09-02", -1, "A")];
        let result = import(&pool, &backups.0, csv_batch(investment_id, "h1"), &lines).await;
        assert!(matches!(
            result,
            Err(AppError::Operation(
                OperationError::AccountWithoutOperations
            ))
        ));

        assert_eq!(count(&pool, "import_batch").await, 0);
        assert_eq!(count(&pool, "operation").await, 0);
        assert!(backups.files().is_empty());
    });
}

#[test]
fn cancelling_removes_the_whole_batch_only() {
    run(|pool, backups| async move {
        let account_id = new_account(&pool, AccountType::Checking).await;
        let manual = operation::create(
            &pool,
            NewOperation {
                account_id,
                date: date("2026-09-05"),
                value_date: None,
                amount_cents: -1000,
                label: "Manuel".to_owned(),
                category_id: None,
                note: None,
                tags: Vec::new(),
            },
        )
        .await
        .unwrap();
        let kept = import(
            &pool,
            &backups.0,
            csv_batch(account_id, "h1"),
            &[line("2026-09-02", -420, "A")],
        )
        .await
        .unwrap();
        let cancelled = import(
            &pool,
            &backups.0,
            csv_batch(account_id, "h2"),
            &[line("2026-09-03", -1, "B"), line("2026-09-04", -2, "C")],
        )
        .await
        .unwrap();

        cancel(&pool, Path::new("/nonexistent/statements"), cancelled.id)
            .await
            .unwrap();

        let ids: Vec<i64> = operation::list(&pool, account_id, None, None)
            .await
            .unwrap()
            .iter()
            .map(|row| row.operation.id)
            .collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&manual.id));
        assert_eq!(count(&pool, "operation_split").await, 2);
        assert_eq!(list(&pool, account_id).await.unwrap(), vec![kept]);

        assert!(matches!(
            cancel(&pool, Path::new("/nonexistent/statements"), cancelled.id).await,
            Err(AppError::Import(ImportError::NotFound(_)))
        ));
    });
}

#[test]
fn lists_batches_and_finds_them_by_hash() {
    run(|pool, backups| async move {
        let account_id = new_account(&pool, AccountType::Checking).await;
        let other_id = new_account(&pool, AccountType::Savings).await;
        let lines = [line("2026-09-02", -420, "A")];
        let first = import(&pool, &backups.0, csv_batch(account_id, "same"), &lines)
            .await
            .unwrap();
        let second = import(&pool, &backups.0, csv_batch(account_id, "other"), &lines)
            .await
            .unwrap();
        let third = import(&pool, &backups.0, csv_batch(other_id, "same"), &lines)
            .await
            .unwrap();

        assert_eq!(
            list(&pool, account_id).await.unwrap(),
            vec![second, first.clone()]
        );
        assert_eq!(
            find_by_hash(&pool, "same").await.unwrap(),
            vec![third, first]
        );
        assert!(find_by_hash(&pool, "none").await.unwrap().is_empty());
        assert!(matches!(
            get(&pool, 999).await,
            Err(AppError::Import(ImportError::NotFound(999)))
        ));
    });
}

#[test]
fn imports_lines_with_their_category() {
    run(|pool, backups| async move {
        let account_id = new_account(&pool, AccountType::Checking).await;
        let categories = crate::operation::category::list(&pool).await.unwrap();
        let transfer = categories
            .iter()
            .find(|c| c.is_internal_transfer)
            .unwrap()
            .id;
        let groceries = categories.iter().find(|c| c.name == "Courses").unwrap().id;
        let lines = [
            ImportLine {
                category_id: Some(groceries),
                ..line("2026-09-02", -420, "CB CARREFOUR")
            },
            ImportLine {
                category_id: Some(transfer),
                ..line("2026-09-03", -10_000, "VIR VERS LIVRET A")
            },
            line("2026-09-04", -100, "CB INCONNU"),
        ];
        import(&pool, &backups.0, csv_batch(account_id, "h1"), &lines)
            .await
            .unwrap();

        let rows = operation::list(&pool, account_id, None, None)
            .await
            .unwrap();
        let found: Vec<(Option<i64>, bool)> = rows
            .iter()
            .rev()
            .map(|row| {
                (
                    row.operation.category_id,
                    row.operation.is_internal_transfer,
                )
            })
            .collect();
        assert_eq!(
            found,
            vec![
                (Some(groceries), false),
                (Some(transfer), true),
                (None, false)
            ]
        );

        let unknown = [ImportLine {
            category_id: Some(999),
            ..line("2026-09-05", -1, "X")
        }];
        assert!(matches!(
            import(&pool, &backups.0, csv_batch(account_id, "h2"), &unknown).await,
            Err(AppError::Operation(OperationError::CategoryNotFound(999)))
        ));
        assert_eq!(list(&pool, account_id).await.unwrap().len(), 1);
    });
}
