use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::error::AppError;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "money-encryption-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn data_file(&self) -> PathBuf {
        self.0.join("money.sqlite")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn account_names(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM account ORDER BY name")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn add_account(pool: &SqlitePool, name: &str) {
    sqlx::query(
        "INSERT INTO account (name, type, opening_balance_cents, opening_date)
         VALUES (?1, 'CHECKING', 0, '2026-01-01')",
    )
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
}

#[test]
fn encrypts_changes_the_password_and_decrypts() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new();
        let file = temp.data_file();
        let pool = crate::db::open(&file).await.unwrap();
        add_account(&pool, "Courant").await;
        assert!(!is_encrypted(&file).unwrap());

        assert!(matches!(
            encrypt(&pool, &file, "court").await,
            Err(AppError::Encryption(EncryptionError::PasswordTooShort(8)))
        ));
        encrypt(&pool, &file, "mot de passe d'O'Brien")
            .await
            .unwrap();
        assert!(pool.is_closed());
        assert!(is_encrypted(&file).unwrap());
        assert!(
            !std::fs::read(&file)
                .unwrap()
                .windows(7)
                .any(|w| w == b"Courant")
        );

        assert!(matches!(
            open(&file, "mauvais mot de passe").await,
            Err(AppError::Encryption(EncryptionError::WrongPassword))
        ));
        let pool = open(&file, "mot de passe d'O'Brien").await.unwrap();
        assert_eq!(account_names(&pool).await, ["Courant"]);
        // Migrations are already applied: the version follows the export.
        let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(applied > 0);

        assert!(matches!(
            change_password(&pool, &file, "faux", "nouveau mot de passe").await,
            Err(AppError::Encryption(EncryptionError::WrongPassword))
        ));
        change_password(
            &pool,
            &file,
            "mot de passe d'O'Brien",
            "nouveau mot de passe",
        )
        .await
        .unwrap();
        let pool = open(&file, "nouveau mot de passe").await.unwrap();
        assert_eq!(account_names(&pool).await, ["Courant"]);

        decrypt(&pool, &file, "nouveau mot de passe").await.unwrap();
        assert!(!is_encrypted(&file).unwrap());
        let pool = crate::db::open(&file).await.unwrap();
        assert_eq!(account_names(&pool).await, ["Courant"]);
        assert!(matches!(
            decrypt(&pool, &file, "nouveau mot de passe").await,
            Err(AppError::Encryption(EncryptionError::NotEncrypted))
        ));
    });
}

#[test]
fn backups_of_an_encrypted_file_stay_encrypted() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new();
        let file = temp.data_file();
        let pool = crate::db::open(&file).await.unwrap();
        add_account(&pool, "Courant").await;
        encrypt(&pool, &file, "mot de passe").await.unwrap();
        let pool = open(&file, "mot de passe").await.unwrap();

        let backup = crate::importing::batch::backup(&pool, &temp.0.join("backups"))
            .await
            .unwrap();
        assert!(is_encrypted(&backup).unwrap());
        let copy = open(&backup, "mot de passe").await.unwrap();
        assert_eq!(account_names(&copy).await, ["Courant"]);
    });
}

#[test]
fn tells_files_in_clear_from_encrypted_ones() {
    let temp = TempDir::new();
    assert!(!is_encrypted(&temp.0.join("absent.sqlite")).unwrap());
    std::fs::write(temp.0.join("vide.sqlite"), b"").unwrap();
    assert!(!is_encrypted(&temp.0.join("vide.sqlite")).unwrap());
    std::fs::write(temp.0.join("chiffre.sqlite"), [7u8; 64]).unwrap();
    assert!(is_encrypted(&temp.0.join("chiffre.sqlite")).unwrap());
}
