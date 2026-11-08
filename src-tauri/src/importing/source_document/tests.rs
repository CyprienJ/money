use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;

/// Statements directory of a test, deleted afterwards.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "money-statements-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn batch(pool: &SqlitePool, stored_file: Option<&str>) -> i64 {
    let account_id = account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: "Courant".to_owned(),
            account_type: AccountType::Checking,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: Some("2026-01-01".parse().unwrap()),
        },
    )
    .await
    .unwrap()
    .id;
    sqlx::query(
        "INSERT INTO import_batch (account_id, source_type, source_file_name, source_file_hash,
                                   imported_at, balance_check_status, stored_file)
         VALUES (?1, 'PDF', 'Relevé.PDF', 'abc', '2026-10-20T10:00:00Z', 'NOT_AVAILABLE', ?2)",
    )
    .bind(account_id)
    .bind(stored_file)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

#[test]
fn names_a_copy_by_its_content() {
    assert_eq!(stored_name("abc", "Relevé Septembre.PDF"), "abc.pdf");
    assert_eq!(stored_name("abc", "export.qif"), "abc.qif");
    assert_eq!(stored_name("abc", "sans extension"), "abc");
    assert_eq!(stored_name("abc", "bizarre.é"), "abc");
}

#[test]
fn keeps_one_copy_per_content_and_sweeps_the_unused_ones() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let dir = TempDir::new();
        assert_eq!(store(&dir.0, "abc", "a.pdf", b"one").unwrap(), "abc.pdf");
        // Same content imported again: the copy stays as it is.
        assert_eq!(store(&dir.0, "abc", "b.pdf", b"other").unwrap(), "abc.pdf");
        assert_eq!(std::fs::read(dir.0.join("abc.pdf")).unwrap(), b"one");
        store(&dir.0, "def", "c.csv", b"two").unwrap();

        let id = batch(&pool, Some("abc.pdf")).await;
        assert_eq!(sweep(&pool, &dir.0).await.unwrap(), 1);
        assert!(dir.0.join("abc.pdf").is_file());
        assert!(!dir.0.join("def.csv").exists());

        let document = get(&pool, id).await.unwrap();
        assert_eq!(document.file_name, "Relevé.PDF");
        assert_eq!(document.kind, DocumentKind::Pdf);
        assert_eq!(
            path_of(&pool, &dir.0, id).await.unwrap(),
            dir.0.join("abc.pdf")
        );
        assert!(matches!(
            text_of(&pool, &dir.0, id).await,
            Err(AppError::SourceDocument(SourceDocumentError::NotText(_)))
        ));

        std::fs::remove_file(dir.0.join("abc.pdf")).unwrap();
        assert!(matches!(
            path_of(&pool, &dir.0, id).await,
            Err(AppError::SourceDocument(SourceDocumentError::FileMissing(
                _
            )))
        ));
        // Swept with a missing directory: nothing to do.
        assert_eq!(sweep(&pool, &dir.0.join("absent")).await.unwrap(), 0);
    });
}

#[test]
fn imports_made_before_keep_no_file() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let id = batch(&pool, None).await;
        assert!(matches!(
            get(&pool, id).await,
            Err(AppError::SourceDocument(SourceDocumentError::NotKept(_)))
        ));
    });
}
