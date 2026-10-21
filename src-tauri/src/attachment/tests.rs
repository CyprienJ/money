use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::NaiveDate;
use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::{self, NewOperation};

/// Fresh directory under the system temporary directory, removed on drop: `sources/` holds
/// the files to attach, `attachments/` their copies.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let name = format!(
            "money-attachment-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("sources")).unwrap();
        TempDir(path)
    }

    fn attachments(&self) -> PathBuf {
        self.0.join(ATTACHMENTS_DIR)
    }

    /// Writes a file to attach and returns its path.
    fn source(&self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.0.join("sources").join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn stored_files(&self) -> Vec<String> {
        let mut names: Vec<String> = match std::fs::read_dir(self.attachments()) {
            Ok(entries) => entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect(),
            Err(_) => Vec::new(),
        };
        names.sort();
        names
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

async fn new_operation(pool: &SqlitePool) -> i64 {
    let account_id = account::create(
        pool,
        NewAccount {
            name: "Courant".to_owned(),
            account_type: AccountType::Checking,
            institution: None,
            opening_balance_cents: 0,
            opening_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        },
    )
    .await
    .unwrap()
    .id;
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: NaiveDate::from_ymd_opt(2026, 10, 8).unwrap(),
            value_date: None,
            amount_cents: -4500,
            label: "Fnac".to_owned(),
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
fn names_the_copies_safely() {
    assert_eq!(stored_name(7, "facture.pdf"), "7-facture.pdf");
    assert_eq!(stored_name(7, "a:b|c?.PDF"), "7-a_b_c_.PDF");
    assert_eq!(stored_name(7, "sans extension"), "7-sans extension");
    assert_eq!(stored_name(7, ".bashrc"), "7-.bashrc");
    let long = format!("{}.pdf", "x".repeat(200));
    assert_eq!(stored_name(7, &long), format!("7-{}.pdf", "x".repeat(80)));
}

#[test]
fn copies_lists_and_opens_attachments() {
    run(|pool, dir| async move {
        let operation_id = new_operation(&pool).await;
        let source = dir.source("Facture Fnac.pdf", b"%PDF-1.7");

        let added = add(&pool, &dir.attachments(), operation_id, &source)
            .await
            .unwrap();
        assert_eq!(added.file_name, "Facture Fnac.pdf");
        assert_eq!(added.size_bytes, 8);
        assert_eq!(
            list(&pool, operation_id).await.unwrap(),
            std::slice::from_ref(&added)
        );
        assert_eq!(
            operation::get(&pool, operation_id)
                .await
                .unwrap()
                .attachment_count,
            1
        );

        // The copy is independent from the source.
        std::fs::remove_file(&source).unwrap();
        let path = path_of(&pool, &dir.attachments(), added.id).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"%PDF-1.7");
        assert_eq!(
            dir.stored_files(),
            [format!("{}-Facture Fnac.pdf", added.id)]
        );
    });
}

#[test]
fn refuses_large_files_and_directories() {
    run(|pool, dir| async move {
        let operation_id = new_operation(&pool).await;
        let large = dir.source("video.mp4", b"");
        std::fs::File::options()
            .write(true)
            .open(&large)
            .unwrap()
            .set_len(MAX_SIZE_BYTES + 1)
            .unwrap();
        assert!(matches!(
            add(&pool, &dir.attachments(), operation_id, &large).await,
            Err(AppError::Attachment(AttachmentError::TooLarge { .. }))
        ));
        assert!(matches!(
            add(
                &pool,
                &dir.attachments(),
                operation_id,
                &dir.0.join("sources")
            )
            .await,
            Err(AppError::Attachment(AttachmentError::NotAFile(_)))
        ));
        let source = dir.source("ticket.png", b"png");
        assert!(matches!(
            add(&pool, &dir.attachments(), 99, &source).await,
            Err(AppError::Operation(_))
        ));
        assert!(dir.stored_files().is_empty());
    });
}

#[test]
fn deletes_an_attachment_with_its_file() {
    run(|pool, dir| async move {
        let operation_id = new_operation(&pool).await;
        let added = add(
            &pool,
            &dir.attachments(),
            operation_id,
            &dir.source("ticket.png", b"png"),
        )
        .await
        .unwrap();
        delete(&pool, &dir.attachments(), added.id).await.unwrap();
        assert!(list(&pool, operation_id).await.unwrap().is_empty());
        assert!(dir.stored_files().is_empty());
        assert!(matches!(
            delete(&pool, &dir.attachments(), added.id).await,
            Err(AppError::Attachment(AttachmentError::NotFound(_)))
        ));
    });
}

#[test]
fn sweeps_the_files_of_deleted_operations() {
    run(|pool, dir| async move {
        assert_eq!(sweep(&pool, &dir.attachments()).await.unwrap(), 0);
        let kept = new_operation(&pool).await;
        let deleted = new_operation(&pool).await;
        let kept_file = add(&pool, &dir.attachments(), kept, &dir.source("a.pdf", b"a"))
            .await
            .unwrap();
        add(
            &pool,
            &dir.attachments(),
            deleted,
            &dir.source("b.pdf", b"b"),
        )
        .await
        .unwrap();

        operation::delete(&pool, deleted).await.unwrap();
        assert_eq!(dir.stored_files().len(), 2);
        assert_eq!(sweep(&pool, &dir.attachments()).await.unwrap(), 1);
        assert_eq!(dir.stored_files(), [format!("{}-a.pdf", kept_file.id)]);
    });
}

#[test]
fn reports_a_missing_file() {
    run(|pool, dir| async move {
        let operation_id = new_operation(&pool).await;
        let added = add(
            &pool,
            &dir.attachments(),
            operation_id,
            &dir.source("a.pdf", b"a"),
        )
        .await
        .unwrap();
        std::fs::remove_dir_all(dir.attachments()).unwrap();
        assert!(matches!(
            path_of(&pool, &dir.attachments(), added.id).await,
            Err(AppError::Attachment(AttachmentError::FileMissing(_)))
        ));
    });
}
