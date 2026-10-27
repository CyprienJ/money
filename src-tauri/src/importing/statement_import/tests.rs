use std::sync::atomic::{AtomicU32, Ordering};

use sqlx::SqlitePool;

use super::*;
use crate::account::{AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::importing::batch::BalanceCheckStatus;
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
    /// Kept statements of the test, deleted afterwards.
    statements: std::path::PathBuf,
}

impl Setup {
    fn file(&self) -> StatementFile<'static> {
        StatementFile {
            account_id: self.account_id,
            file_name: "releve.csv",
            bytes: FILE.as_bytes(),
            format: ImportFormat::Csv {
                mapping_id: self.mapping_id,
            },
            pdfium_dir: Path::new(""),
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
                currency: "EUR".to_owned(),
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
                    column_fee: None,
                    state_filter: None,
                },
            },
        )
        .await
        .unwrap()
        .id;
        // Backups of an in-memory database are written in memory: the directory stays empty.
        let backups = std::env::temp_dir().join("money-csv-import-test-backups");
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let statements = std::env::temp_dir().join(format!(
            "money-import-test-statements-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        test(Setup {
            pool,
            account_id,
            mapping_id,
            backups,
            statements: statements.clone(),
        })
        .await;
        let _ = std::fs::remove_dir_all(statements);
    });
}

/// Selections keeping the category proposed by the preview.
async fn proposed(setup: &Setup, indexes: &[u32]) -> Vec<ImportSelection> {
    let preview = preview(&setup.pool, setup.file()).await.unwrap();
    indexes
        .iter()
        .map(|&index| ImportSelection {
            index,
            category_id: preview
                .lines
                .get(index as usize)
                .and_then(|l| l.category_id),
        })
        .collect()
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
            note: None,
            tags: Vec::new(),
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
        assert_eq!(lines[3].normalized_label, "SALAIRE");

        let matched: Vec<i64> = preview.matched_operations.iter().map(|o| o.id).collect();
        assert_eq!(matched, vec![exact, manual]);
    });
}

#[test]
fn imports_the_chosen_lines_once() {
    run(|setup| async move {
        let hash = batch::file_hash(FILE.as_bytes());
        let selections = proposed(&setup, &[2, 3, 3]).await;
        let batch = import(
            &setup.pool,
            &setup.backups,
            &setup.statements,
            setup.file(),
            &hash,
            &selections,
            &[],
        )
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
            async move {
                let selections = proposed(setup, &included).await;
                import(
                    &setup.pool,
                    &setup.backups,
                    &setup.statements,
                    setup.file(),
                    &hash,
                    &selections,
                    &[],
                )
                .await
            }
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

#[test]
fn proposes_and_applies_remembered_categories() {
    run(|setup| async move {
        let categories = crate::operation::category::list(&setup.pool).await.unwrap();
        let income = categories.iter().find(|c| c.name == "Salaire").unwrap().id;
        // A salary categorized by hand earlier, with another date in its raw label.
        let earlier = enter(
            &setup.pool,
            setup.account_id,
            "2026-09-01",
            245_000,
            "VIR SALAIRE 01/09",
        )
        .await;
        crate::operation::set_category(&setup.pool, earlier, Some(income))
            .await
            .unwrap();

        let preview = preview(&setup.pool, setup.file()).await.unwrap();
        let categories: Vec<Option<i64>> = preview.lines.iter().map(|l| l.category_id).collect();
        assert_eq!(categories, vec![None, None, None, Some(income)]);

        let hash = batch::file_hash(FILE.as_bytes());
        let selections = proposed(&setup, &[2, 3]).await;
        import(
            &setup.pool,
            &setup.backups,
            &setup.statements,
            setup.file(),
            &hash,
            &selections,
            &[],
        )
        .await
        .unwrap();
        let rows = crate::operation::list(
            &setup.pool,
            setup.account_id,
            Some(date("2026-09-02")),
            None,
        )
        .await
        .unwrap();
        let imported: Vec<(String, Option<i64>)> = rows
            .iter()
            .map(|r| {
                (
                    r.operation.normalized_label.clone(),
                    r.operation.category_id,
                )
            })
            .collect();
        assert_eq!(
            imported,
            vec![
                ("SALAIRE".to_owned(), Some(income)),
                ("RESTAURANT".to_owned(), None)
            ]
        );
    });
}

#[test]
fn rules_come_before_remembered_categories() {
    run(|setup| async move {
        let categories = crate::operation::category::list(&setup.pool).await.unwrap();
        let id = |name: &str| categories.iter().find(|c| c.name == name).unwrap().id;
        let (salary, other_income, restaurants) =
            (id("Salaire"), id("Autres revenus"), id("Restaurants"));
        let earlier = enter(
            &setup.pool,
            setup.account_id,
            "2026-09-01",
            245_000,
            "VIR SALAIRE 01/09",
        )
        .await;
        crate::operation::set_category(&setup.pool, earlier, Some(salary))
            .await
            .unwrap();
        for (pattern, category_id) in [("salaire", other_income), ("restau", restaurants)] {
            crate::labeling::rules::create(
                &setup.pool,
                crate::labeling::rules::RuleInput {
                    pattern: pattern.to_owned(),
                    category_id,
                },
            )
            .await
            .unwrap();
        }

        let preview = preview(&setup.pool, setup.file()).await.unwrap();
        let proposed: Vec<(Option<i64>, Option<&str>)> = preview
            .lines
            .iter()
            .map(|l| (l.category_id, l.rule_pattern.as_deref()))
            .collect();
        assert_eq!(
            proposed,
            vec![
                (None, None),
                (None, None),
                (Some(restaurants), Some("restau")),
                // The rule wins over the category remembered for the label.
                (Some(other_income), Some("salaire")),
            ]
        );
    });
}

#[test]
fn remembers_categories_chosen_in_the_preview() {
    run(|setup| async move {
        let categories = crate::operation::category::list(&setup.pool).await.unwrap();
        let restaurants = categories
            .iter()
            .find(|c| c.name == "Restaurants" && c.parent_id.is_none())
            .unwrap()
            .id;
        let salary = categories.iter().find(|c| c.name == "Salaire").unwrap().id;
        let earlier = enter(
            &setup.pool,
            setup.account_id,
            "2026-09-01",
            245_000,
            "VIR SALAIRE 01/09",
        )
        .await;
        crate::operation::set_category(&setup.pool, earlier, Some(salary))
            .await
            .unwrap();

        let hash = batch::file_hash(FILE.as_bytes());
        let selections = [
            // Unknown label: chosen by the user.
            ImportSelection {
                index: 2,
                category_id: Some(restaurants),
            },
            // Proposed category kept.
            ImportSelection {
                index: 3,
                category_id: Some(salary),
            },
            // Left to categorize.
            ImportSelection {
                index: 1,
                category_id: None,
            },
        ];
        import(
            &setup.pool,
            &setup.backups,
            &setup.statements,
            setup.file(),
            &hash,
            &selections,
            &[],
        )
        .await
        .unwrap();

        let mut conn = setup.pool.acquire().await.unwrap();
        let remembered = memory::recall(&mut conn, &["RESTAURANT", "SALAIRE", "BOULANGERIE"])
            .await
            .unwrap();
        assert_eq!(remembered.get("RESTAURANT"), Some(&restaurants));
        assert_eq!(remembered.get("SALAIRE"), Some(&salary));
        assert_eq!(remembered.get("BOULANGERIE"), None);
    });
}

#[test]
fn previews_and_imports_a_pdf_statement() {
    run(|setup| async move {
        let path = format!(
            "{}/../tests/fixtures/fortuneo.pdf",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(path).unwrap();
        let pdfium_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/pdfium"));
        let file = StatementFile {
            account_id: setup.account_id,
            file_name: "fortuneo.pdf",
            bytes: &bytes,
            format: ImportFormat::Pdf,
            pdfium_dir,
        };

        let preview = preview(&setup.pool, file).await.unwrap();
        assert_eq!(preview.bank.as_deref(), Some("Fortuneo"));
        assert_eq!(preview.start_balance_cents, Some(797));
        assert_eq!(preview.end_balance_cents, Some(8_717));
        assert_eq!(preview.first_date, Some(date("2026-09-01")));
        assert_eq!(preview.last_date, Some(date("2026-09-30")));
        assert_eq!(preview.lines.len(), 2);
        assert_eq!(preview.lines[0].normalized_label, "MAXICOFFEE SUD SATURNE");
        assert_eq!(
            preview.balance_check,
            BalanceCheck {
                status: BalanceCheckStatus::Ok,
                lines_total_cents: 7_920,
                computed_end_cents: Some(8_717),
            }
        );

        // Only one line kept: the check still covers the whole statement.
        let selections = [ImportSelection {
            index: 1,
            category_id: None,
        }];
        let batch = import(
            &setup.pool,
            &setup.backups,
            &setup.statements,
            file,
            &preview.file_hash,
            &selections,
            &[],
        )
        .await
        .unwrap();
        assert_eq!(batch.source_type, SourceType::Pdf);
        assert_eq!(batch.operation_count, 1);
        assert_eq!(batch.statement_start_balance_cents, Some(797));
        assert_eq!(batch.statement_end_balance_cents, Some(8_717));
        assert_eq!(batch.balance_check_status, BalanceCheckStatus::Ok);
    });
}

#[test]
fn refuses_pdfs_of_unsupported_banks() {
    run(|setup| async move {
        let pdfium_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/pdfium"));
        // A minimal valid PDF with no statement in it.
        let bytes = b"%PDF-1.1\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF";
        let file = StatementFile {
            account_id: setup.account_id,
            file_name: "autre.pdf",
            bytes,
            format: ImportFormat::Pdf,
            pdfium_dir,
        };
        assert!(matches!(
            preview(&setup.pool, file).await,
            Err(AppError::Pdf(PdfError::UnknownFormat))
        ));
    });
}

const QIF: &str = "!Type:Bank
D02/09/26
T-64,28
PCarrefour
MCourses de la semaine
LCourses:Alimentation
^
D03/09/26
T-12,00
PCinema
LLoisirs
^
D04/09/26
T-2,50
PFrais tenue
LBanque:Frais
^
D05/09/26
T-300,00
PVirement epargne
L[Livret A]
^
";

fn qif_file(setup: &Setup) -> StatementFile<'static> {
    StatementFile {
        file_name: "courant.qif",
        bytes: QIF.as_bytes(),
        format: ImportFormat::Qif,
        ..setup.file()
    }
}

#[test]
fn imports_a_qif_file_with_its_categories() {
    run(|setup| async move {
        let categories = category::list(&setup.pool).await.unwrap();
        let id_of = |name: &str, parent: Option<&str>| {
            let parent_id = parent.map(|p| {
                categories
                    .iter()
                    .find(|c| c.name == p && c.parent_id.is_none())
                    .unwrap()
                    .id
            });
            categories
                .iter()
                .find(|c| c.name == name && c.parent_id == parent_id)
                .unwrap()
                .id
        };
        let food = id_of("Alimentation", Some("Courses"));
        let leisure = id_of("Loisirs", None);
        let other = id_of("Autres", None);
        let transfer = categories
            .iter()
            .find(|c| c.is_internal_transfer)
            .unwrap()
            .id;

        let preview = preview(&setup.pool, qif_file(&setup)).await.unwrap();
        let found: Vec<(&str, Option<i64>, u32)> = preview
            .source_categories
            .iter()
            .map(|c| (c.name.as_str(), c.category_id, c.line_count))
            .collect();
        assert_eq!(
            found,
            [
                ("Banque:Frais", None, 1),
                ("Courses:Alimentation", Some(food), 1),
                ("Loisirs", Some(leisure), 1),
            ]
        );
        let proposed: Vec<Option<i64>> = preview.lines.iter().map(|l| l.category_id).collect();
        assert_eq!(proposed, [Some(food), Some(leisure), None, Some(transfer)]);

        let selections: Vec<ImportSelection> =
            [Some(food), Some(leisure), Some(other), Some(transfer)]
                .into_iter()
                .enumerate()
                .map(|(index, category_id)| ImportSelection {
                    index: index as u32,
                    category_id,
                })
                .collect();
        let choices = [SourceCategoryChoice {
            name: "Banque:Frais".to_owned(),
            category_id: Some(other),
        }];
        let batch = import(
            &setup.pool,
            &setup.backups,
            &setup.statements,
            qif_file(&setup),
            &preview.file_hash,
            &selections,
            &choices,
        )
        .await
        .unwrap();
        assert_eq!(batch.source_type, SourceType::Qif);
        assert_eq!(batch.operation_count, 4);

        let note: Option<String> =
            sqlx::query_scalar("SELECT note FROM operation WHERE raw_label = 'Carrefour'")
                .fetch_one(&setup.pool)
                .await
                .unwrap();
        assert_eq!(note.as_deref(), Some("Courses de la semaine"));
        let transfers: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM operation WHERE is_internal_transfer = 1")
                .fetch_one(&setup.pool)
                .await
                .unwrap();
        assert_eq!(transfers, 1);
        // The category given by the file is remembered for the label.
        let mut conn = setup.pool.acquire().await.unwrap();
        let remembered = memory::recall(&mut conn, &["CARREFOUR"]).await.unwrap();
        assert_eq!(remembered.get("CARREFOUR"), Some(&food));
        drop(conn);

        // The choice is remembered for the next files.
        let again = super::preview(&setup.pool, qif_file(&setup)).await.unwrap();
        let fees = &again.source_categories[0];
        assert_eq!((fees.category_id, fees.remembered), (Some(other), true));
    });
}

#[test]
fn refuses_an_ofx_file_in_another_currency() {
    run(|setup| async move {
        let ofx = "<OFX><STMTRS><CURDEF>USD<BANKTRANLIST>\
                   <STMTTRN><DTPOSTED>20260902<TRNAMT>-1.00<NAME>X</STMTTRN>\
                   </BANKTRANLIST></STMTRS></OFX>";
        let file = StatementFile {
            file_name: "releve.ofx",
            bytes: ofx.as_bytes(),
            format: ImportFormat::Ofx,
            ..setup.file()
        };
        assert!(matches!(
            preview(&setup.pool, file).await,
            Err(AppError::Interchange(InterchangeError::CurrencyMismatch { file, account }))
                if file == "USD" && account == "EUR"
        ));
    });
}

#[test]
fn keeps_the_imported_file_for_its_operations() {
    run(|setup| async move {
        let hash = batch::file_hash(FILE.as_bytes());
        let selections = proposed(&setup, &[3]).await;
        let batch = import(
            &setup.pool,
            &setup.backups,
            &setup.statements,
            setup.file(),
            &hash,
            &selections,
            &[],
        )
        .await
        .unwrap();
        let kept = setup.statements.join(format!("{hash}.csv"));
        assert!(kept.is_file());

        let operation_id: i64 =
            sqlx::query_scalar("SELECT id FROM operation WHERE import_batch_id = ?1")
                .bind(batch.id)
                .fetch_one(&setup.pool)
                .await
                .unwrap();
        let operation = operation::get(&setup.pool, operation_id).await.unwrap();
        assert!(operation.has_source_document);
        // VIR SALAIRE, the fourth line of the file.
        assert_eq!(
            (operation.source_page, operation.source_line),
            (None, Some(4))
        );

        let document = source_document::get(&setup.pool, batch.id).await.unwrap();
        assert_eq!(document.file_name, "releve.csv");
        assert_eq!(document.kind, source_document::DocumentKind::Text);
        let text = source_document::text_of(&setup.pool, &setup.statements, batch.id)
            .await
            .unwrap();
        assert_eq!(text.lines().nth(3), Some("2026-09-05;VIR SALAIRE;2450.00"));

        // Cancelling the import removes the file nobody uses anymore.
        batch::cancel(&setup.pool, &setup.statements, batch.id)
            .await
            .unwrap();
        assert!(!kept.exists());
    });
}
