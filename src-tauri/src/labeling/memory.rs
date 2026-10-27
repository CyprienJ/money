//! Category memory: the last category the user gave to a normalized label, reused at import.

use std::collections::HashMap;

use sqlx::SqliteConnection;

use crate::error::AppResult;

/// Remembers `category_id` for `normalized_label` after a manual categorization. Clearing a
/// category (`None`) keeps what was remembered.
pub async fn remember(
    conn: &mut SqliteConnection,
    normalized_label: &str,
    category_id: Option<i64>,
    now: &str,
) -> AppResult<()> {
    let Some(category_id) = category_id else {
        return Ok(());
    };
    sqlx::query!(
        "INSERT INTO label_category_memory (normalized_label, category_id, last_used_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT (normalized_label)
         DO UPDATE SET category_id = excluded.category_id, last_used_at = excluded.last_used_at",
        normalized_label,
        category_id,
        now
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Remembered category of each known label among `normalized_labels`.
pub async fn recall(
    conn: &mut SqliteConnection,
    normalized_labels: &[&str],
) -> AppResult<HashMap<String, i64>> {
    let mut categories = HashMap::new();
    for &label in normalized_labels {
        if categories.contains_key(label) {
            continue;
        }
        let category_id = sqlx::query_scalar!(
            "SELECT category_id FROM label_category_memory WHERE normalized_label = ?1",
            label
        )
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(category_id) = category_id {
            categories.insert(label.to_owned(), category_id);
        }
    }
    Ok(categories)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sqlx::SqlitePool;

    use super::*;
    use crate::account::{self, AccountType, NewAccount};
    use crate::db::tests::memory_pool;
    use crate::operation::{self, NewOperation, OperationUpdate, category};

    async fn category_id(pool: &SqlitePool, name: &str) -> i64 {
        category::list(pool)
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.name == name)
            .unwrap()
            .id
    }

    async fn recall_one(pool: &SqlitePool, label: &str) -> Option<i64> {
        let mut conn = pool.acquire().await.unwrap();
        recall(&mut conn, &[label])
            .await
            .unwrap()
            .get(label)
            .copied()
    }

    #[test]
    fn remembers_the_last_category_of_a_label() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let groceries = category_id(&pool, "Courses").await;
            let health = category_id(&pool, "Santé").await;
            let mut conn = pool.acquire().await.unwrap();

            remember(
                &mut conn,
                "CARREFOUR",
                Some(groceries),
                "2026-10-01T10:00:00Z",
            )
            .await
            .unwrap();
            remember(&mut conn, "CARREFOUR", Some(health), "2026-10-02T10:00:00Z")
                .await
                .unwrap();
            remember(&mut conn, "CARREFOUR", None, "2026-10-03T10:00:00Z")
                .await
                .unwrap();
            let found = recall(&mut conn, &["CARREFOUR", "INCONNU", "CARREFOUR"])
                .await
                .unwrap();
            assert_eq!(found.len(), 1);
            assert_eq!(found["CARREFOUR"], health);
        });
    }

    #[test]
    fn manual_categorizations_are_remembered() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let groceries = category_id(&pool, "Courses").await;
            let leisure = category_id(&pool, "Loisirs").await;
            let account_id = account::create(
                &pool,
                NewAccount {
                    name: "Courant".to_owned(),
                    account_type: AccountType::Checking,
                    currency: "EUR".to_owned(),
                    institution: None,
                    opening_balance_cents: 0,
                    opening_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                },
            )
            .await
            .unwrap()
            .id;
            let new = |label: &str, category_id| NewOperation {
                account_id,
                note: None,
                tags: Vec::new(),
                date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                value_date: None,
                amount_cents: -500,
                label: label.to_owned(),
                category_id,
            };

            // At creation, under the normalized label.
            operation::create(&pool, new("CB CARREFOUR 01/09", Some(groceries)))
                .await
                .unwrap();
            assert_eq!(recall_one(&pool, "CARREFOUR").await, Some(groceries));

            // When changing the category.
            let fnac = operation::create(&pool, new("CB FNAC 02/09", None))
                .await
                .unwrap();
            assert_eq!(recall_one(&pool, "FNAC").await, None);
            operation::set_category(&pool, fnac.id, Some(leisure))
                .await
                .unwrap();
            assert_eq!(recall_one(&pool, "FNAC").await, Some(leisure));

            // When editing the operation, under its new label.
            operation::update(
                &pool,
                fnac.id,
                OperationUpdate {
                    date: fnac.date,
                    value_date: None,
                    amount_cents: fnac.amount_cents,
                    label: "CB FNAC.COM".to_owned(),
                    category_id: Some(groceries),
                    note: None,
                    tags: Vec::new(),
                },
            )
            .await
            .unwrap();
            assert_eq!(recall_one(&pool, "FNAC.COM").await, Some(groceries));

            // Clearing the category keeps the memory.
            operation::set_category(&pool, fnac.id, None).await.unwrap();
            assert_eq!(recall_one(&pool, "FNAC.COM").await, Some(groceries));
        });
    }
}
