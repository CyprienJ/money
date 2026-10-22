use std::path::PathBuf;

use super::*;
use crate::account::NewAccount;
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::investment::{self, QUANTITY_SCALE};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

/// PDFium downloaded by `tools/fetch_pdfium.sh`.
fn library_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/pdfium"))
}

fn statement() -> Vec<u8> {
    let path = format!(
        "{}/../tests/fixtures/trade_republic.pdf",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).unwrap()
}

async fn new_account(pool: &SqlitePool, account_type: AccountType) -> i64 {
    account::create(
        pool,
        NewAccount {
            name: "PEA".to_owned(),
            account_type,
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

#[test]
fn imports_the_trades_into_the_portfolio() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let pea = new_account(&pool, AccountType::Investment).await;
        let bytes = statement();
        let pdfium = library_dir();
        let file = TradeFile {
            account_id: pea,
            file_name: "releve.pdf",
            bytes: &bytes,
            pdfium_dir: &pdfium,
        };

        // ASML already entered by hand, fees apart.
        let asml = securities::create(
            &pool,
            SecurityInput {
                name: "ASML".to_owned(),
                isin: Some("NL0010273215".to_owned()),
                symbol: None,
                currency: "EUR".to_owned(),
            },
        )
        .await
        .unwrap()
        .id;
        transactions::create(
            &pool,
            pea,
            TransactionInput {
                security_id: asml,
                date: date("2026-07-27"),
                kind: TransactionKind::Buy,
                quantity_micro: QUANTITY_SCALE,
                amount_cents: 158_100,
                fees_cents: 100,
                note: None,
            },
        )
        .await
        .unwrap();

        let preview = preview(&pool, file).await.unwrap();
        assert_eq!(preview.bank, "Trade Republic");
        assert_eq!(preview.lines.len(), 11);
        let duplicates: Vec<&str> = preview
            .lines
            .iter()
            .filter(|l| l.duplicate)
            .map(|l| l.trade.isin.as_str())
            .collect();
        assert_eq!(duplicates, ["NL0010273215"]);
        assert_eq!(preview.lines[1].security_id, Some(asml));
        assert!(preview.lines[0].security_id.is_none());

        let backups = std::env::temp_dir().join("money-trade-import-test-backups");
        let statements = std::env::temp_dir().join(format!(
            "money-trade-import-test-statements-{}",
            std::process::id()
        ));
        assert!(matches!(
            import(&pool, &backups, &statements, file, "autre", &[0]).await,
            Err(AppError::Import(ImportError::FileChanged))
        ));
        assert!(matches!(
            import(&pool, &backups, &statements, file, &preview.file_hash, &[1]).await,
            Err(AppError::Import(ImportError::DuplicateLine(1)))
        ));
        let kept: Vec<u32> = (0..11).filter(|i| *i != 1).collect();
        let outcome = import(
            &pool,
            &backups,
            &statements,
            file,
            &preview.file_hash,
            &kept,
        )
        .await
        .unwrap();
        assert_eq!(
            outcome,
            TradeImport {
                imported: 10,
                created_securities: 10
            }
        );

        let positions = investment::positions(&pool, pea, date("2026-10-19"))
            .await
            .unwrap();
        assert_eq!(positions.len(), 11);
        let engie = positions
            .iter()
            .find(|p| p.isin.as_deref() == Some("FR0010208488"))
            .unwrap();
        assert_eq!(engie.security_name, "ENGIE S.A. INH. EO 1");
        assert_eq!(engie.quantity_micro, 10 * QUANTITY_SCALE);
        assert_eq!(engie.cost_cents, 26_294);

        // Each trade opens its statement, at its page.
        let imported = transactions::list(&pool, pea).await.unwrap();
        let carrefour = imported
            .iter()
            .find(|t| t.security_name == "CARREFOUR S.A. INH.EO 2,5")
            .unwrap();
        assert!(carrefour.has_source_document);
        assert_eq!(carrefour.source_page, Some(4));
        let document =
            crate::importing::source_document::get(&pool, carrefour.import_batch_id.unwrap())
                .await
                .unwrap();
        assert_eq!(
            document.kind,
            crate::importing::source_document::DocumentKind::Pdf
        );
        assert!(
            !imported
                .iter()
                .find(|t| t.security_id == asml)
                .unwrap()
                .has_source_document
        );
        let _ = std::fs::remove_dir_all(&statements);

        // Imported once: everything is a duplicate now.
        let again = super::preview(&pool, file).await.unwrap();
        assert!(again.lines.iter().all(|l| l.duplicate));
    });
}

#[test]
fn imports_trades_into_investment_accounts_only() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking).await;
        let bytes = statement();
        let pdfium = library_dir();
        let file = TradeFile {
            account_id: checking,
            file_name: "releve.pdf",
            bytes: &bytes,
            pdfium_dir: &pdfium,
        };
        assert!(matches!(
            preview(&pool, file).await,
            Err(AppError::Investment(
                InvestmentError::NotAnInvestmentAccount
            ))
        ));
    });
}
