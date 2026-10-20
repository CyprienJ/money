//! SQLite connection pool and repositories.

use std::path::Path;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::error::AppResult;

/// File name of the data file inside the application data directory.
pub const DATA_FILE_NAME: &str = "money.sqlite";

/// Opens (creating it if needed) the data file and applies pending migrations.
pub async fn open(path: &Path) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    connect(options, SqlitePoolOptions::new()).await
}

async fn connect(
    options: SqliteConnectOptions,
    pool_options: SqlitePoolOptions,
) -> AppResult<SqlitePool> {
    let pool = pool_options
        .connect_with(options.foreign_keys(true))
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// In-memory database with all migrations applied, for unit tests.
    /// A single connection is used because each in-memory connection is a distinct database.
    pub async fn memory_pool() -> SqlitePool {
        let options = SqliteConnectOptions::new().in_memory(true);
        connect(options, SqlitePoolOptions::new().max_connections(1))
            .await
            .expect("in-memory database should open")
    }

    #[test]
    fn enables_foreign_keys() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(enabled, 1);
        });
    }

    #[test]
    fn creates_file_and_applies_migrations() {
        tauri::async_runtime::block_on(async {
            let dir = std::env::temp_dir().join(format!("money-db-test-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join(DATA_FILE_NAME);

            let pool = open(&path).await.unwrap();
            let migrations_table: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = '_sqlx_migrations'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            pool.close().await;

            assert!(path.exists());
            assert_eq!(migrations_table, 1);
            std::fs::remove_dir_all(&dir).unwrap();
        });
    }
}
