//! Application settings kept in the data file (`app_setting`), by key.

use sqlx::SqlitePool;

use crate::error::AppResult;

pub async fn get(pool: &SqlitePool, key: &str) -> AppResult<Option<String>> {
    let value = sqlx::query_scalar!("SELECT value FROM app_setting WHERE key = ?1", key)
        .fetch_optional(pool)
        .await?;
    Ok(value)
}

pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        key,
        value
    )
    .execute(pool)
    .await?;
    Ok(())
}
