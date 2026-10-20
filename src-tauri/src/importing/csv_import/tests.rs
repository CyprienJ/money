use sqlx::SqlitePool;

use super::*;
use crate::account::{AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::importing::csv::{
    AmountColumns, CsvEncoding, CsvMapping, DecimalSeparator, NewCsvMapping,
};
use crate::importing::duplicates::AmbiguityReason;
use crate::operation::{self, NewOperation, OperationError};

const FILE: &str = "\
2026-08-30;AVANT OUVERTURE;-5.00
2026-09-02;CB BOULANGERIE;-4.20
2026-09-03;CB RESTAURANT;-35.00
2026-09-05;VIR SALAIRE;2450.00
";

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

struct Setup {
    pool: SqlitePool,
    account_id: i64,
    mapping_id: i64,
    backups: std::path::PathBuf,
}

impl Setup {
    fn file(&self) -> CsvFile<'static> {
        CsvFile {
            account_id: self.account_id,
            mapping_id: self.mapping_id,
            file_name: "releve.csv",
            bytes: FILE.as_bytes(),
        }
    }
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(Setup) -> F) {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = account::create(
            &pool,
            NewAccount {
                name: "Courant".to_owned(),
                account_type: AccountType::Checking,
                institution: None,
                opening_balance_cents: 0,
                opening_date: date("2026-09-01"),
            },
        )
        .await
        .unwrap()
        .id;
        let mapping_id = csv::create(
            &pool,
            NewCsvMapping {
                name: "Banque".to_owned(),
                mapping: CsvMapping {
                    delimiter: ';',
                    encoding: CsvEncoding::Utf8,
                    date_format: "%Y-%m-%d".to_owned(),
                    decimal_separator: DecimalSeparator::Dot,
                    column_date: 0,
                    column_label: 1,
                    amount_columns: AmountColumns::Signed { amount: 2 },
                    skip_lines: 0,
                },
            },
        )
        .await
        .unwrap()
        .id;
        // Backups of an in-memory database are written in memory: the directory stays empty.
        let backups = std::env::temp_dir().join("money-csv-import-test-backups");
        test(Setup {
            pool,
            account_id,
            mapping_id,
            backups,
        })
        .await;
    });
}

async fn enter(pool: &SqlitePool, account_id: i64, day: &str, cents: i64, label: &str) -> i64 {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents: cents,
            label: label.to_owned(),
            category_id: None,
        },
    )
    .await
    .unwrap()
    .id
}

#[test]
fn previews_lines_with_their_status() {
    run(|setup| async move {
        let exact = enter(
            &setup.pool,
            setup.account_id,
            "2026-09-02",
            -420,
            "CB BOULANGERIE",
        )
        .await;
        let manual = enter(&setup.pool, setup.account_id, "2026-09-04", -3500, "Resto").await;
        enter(
            &setup.pool,
            setup.account_id,
            "2026-10-20",
            -3500,
            "Plus tard",
        )
        .await;

        let preview = preview(&setup.pool, setup.file()).await.unwrap();
        assert_eq!(preview.file_name, "releve.csv");
        assert_eq!(preview.file_hash, batch::file_hash(FILE.as_bytes()));
        assert_eq!(preview.first_date, Some(date("2026-08-30")));
        assert_eq!(preview.last_date, Some(date("2026-09-05")));
        assert!(preview.previous_imports.is_empty());

        let lines = &preview.lines;
        assert_eq!(lines.len(), 4);
        assert!(lines[0].before_opening && !lines[0].included_by_default);
        assert_eq!(
            lines[1].duplicate,
            DuplicateStatus::Duplicate {
                operation_id: exact
            }
        );
        assert!(!lines[1].included_by_default);
        assert_eq!(
            lines[2].duplicate,
            DuplicateStatus::Ambiguous {
                reason: AmbiguityReason::NearManualEntry,
                candidate_ids: vec![manual],
            }
        );
        assert!(!lines[2].included_by_default);
        assert_eq!(lines[3].duplicate, DuplicateStatus::New);
        assert!(lines[3].included_by_default);
        assert_eq!(lines[3].normalized_label, "VIR SALAIRE");

        let matched: Vec<i64> = preview.matched_operations.iter().map(|o| o.id).collect();
        assert_eq!(matched, vec![exact, manual]);
    });
}

#[test]
fn imports_the_chosen_lines_once() {
    run(|setup| async move {
        let hash = batch::file_hash(FILE.as_bytes());
        let batch = import(&setup.pool, &setup.backups, setup.file(), &hash, &[2, 3, 3])
            .await
            .unwrap();
        assert_eq!(batch.operation_count, 2);
        assert_eq!(batch.source_file_name, "releve.csv");
        assert_eq!(batch.balance_check_status, BalanceCheckStatus::NotAvailable);

        let again = preview(&setup.pool, setup.file()).await.unwrap();
        assert_eq!(again.previous_imports, vec![batch]);
        assert!(matches!(
            again.lines[2].duplicate,
            DuplicateStatus::Duplicate { .. }
        ));
        assert!(matches!(
            again.lines[3].duplicate,
            DuplicateStatus::Duplicate { .. }
        ));
        assert_eq!(again.lines[1].duplicate, DuplicateStatus::New);
    });
}

#[test]
fn refuses_invalid_selections() {
    run(|setup| async move {
        let hash = batch::file_hash(FILE.as_bytes());
        enter(
            &setup.pool,
            setup.account_id,
            "2026-09-02",
            -420,
            "CB BOULANGERIE",
        )
        .await;
        let import_lines = |included: Vec<u32>, hash: String| {
            let setup = &setup;
            async move { import(&setup.pool, &setup.backups, setup.file(), &hash, &included).await }
        };

        assert!(matches!(
            import_lines(vec![3], "other".to_owned()).await,
            Err(AppError::Import(ImportError::FileChanged))
        ));
        assert!(matches!(
            import_lines(vec![4], hash.clone()).await,
            Err(AppError::Import(ImportError::LineOutOfRange(4)))
        ));
        assert!(matches!(
            import_lines(vec![1], hash.clone()).await,
            Err(AppError::Import(ImportError::DuplicateLine(1)))
        ));
        assert!(matches!(
            import_lines(vec![0], hash.clone()).await,
            Err(AppError::Operation(OperationError::DateBeforeOpening(_)))
        ));
        assert!(matches!(
            import_lines(vec![], hash).await,
            Err(AppError::Import(ImportError::NoLine))
        ));
        assert!(
            batch::list(&setup.pool, setup.account_id)
                .await
                .unwrap()
                .is_empty()
        );
    });
}
