//! Other operations sharing the normalized label of an operation, to spread a category the user
//! just gave to one of them.

use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::repository::{internal_transfer_flag, timestamp};
use crate::error::AppResult;

/// Other operations with the same normalized label as an operation, all accounts included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SameLabel {
    pub normalized_label: String,
    /// Others still to categorize.
    pub uncategorized: u32,
    /// Others in another category than the operation, by category, the largest first.
    pub other_categories: Vec<CategoryCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    pub category_id: i64,
    pub count: u32,
}

/// Other operations sharing the label of operation `id`.
pub async fn same_label(pool: &SqlitePool, id: i64) -> AppResult<SameLabel> {
    let operation = super::get(pool, id).await?;
    let rows = sqlx::query!(
        r#"SELECT s.category_id, COUNT(*) AS "count!: u32"
           FROM operation o
           JOIN operation_split s ON s.operation_id = o.id
           WHERE o.normalized_label = ?1 AND o.id <> ?2
           GROUP BY s.category_id
           ORDER BY COUNT(*) DESC, s.category_id"#,
        operation.normalized_label,
        id
    )
    .fetch_all(pool)
    .await?;

    let mut result = SameLabel {
        normalized_label: operation.normalized_label,
        uncategorized: 0,
        other_categories: Vec::new(),
    };
    for row in rows {
        match row.category_id {
            None => result.uncategorized = row.count,
            Some(category_id) if Some(category_id) != operation.category_id => {
                result.other_categories.push(CategoryCount {
                    category_id,
                    count: row.count,
                });
            }
            Some(_) => {}
        }
    }
    Ok(result)
}

/// Gives the category of operation `id` to the other operations of its label still to
/// categorize; returns their count. Operations already categorized are left as they are.
pub async fn categorize_same_label(pool: &SqlitePool, id: i64) -> AppResult<u32> {
    let operation = super::get(pool, id).await?;
    let Some(category_id) = operation.category_id else {
        return Ok(0);
    };
    let mut tx = pool.begin().await?;
    let is_internal_transfer = internal_transfer_flag(&mut tx, Some(category_id)).await?;
    let now = timestamp();

    // The flag first: the splits still tell which operations are to categorize.
    sqlx::query!(
        "UPDATE operation SET is_internal_transfer = ?3, updated_at = ?4
         WHERE normalized_label = ?1 AND id <> ?2
           AND id IN (SELECT operation_id FROM operation_split WHERE category_id IS NULL)",
        operation.normalized_label,
        id,
        is_internal_transfer,
        now
    )
    .execute(&mut *tx)
    .await?;
    let updated = sqlx::query!(
        "UPDATE operation_split SET category_id = ?3
         WHERE category_id IS NULL
           AND operation_id IN (SELECT id FROM operation WHERE normalized_label = ?1 AND id <> ?2)",
        operation.normalized_label,
        id,
        category_id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(u32::try_from(updated).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::account::{self, AccountType, NewAccount};
    use crate::db::tests::memory_pool;
    use crate::operation::{self, NewOperation, category};

    async fn category_id(pool: &SqlitePool, name: &str) -> i64 {
        category::list(pool)
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.name == name)
            .unwrap()
            .id
    }

    async fn new_account(pool: &SqlitePool, name: &str) -> i64 {
        account::create(
            pool,
            NewAccount {
                name: name.to_owned(),
                account_type: AccountType::Checking,
                institution: None,
                opening_balance_cents: 0,
                opening_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn new_operation(
        pool: &SqlitePool,
        account_id: i64,
        label: &str,
        category_id: Option<i64>,
    ) -> i64 {
        operation::create(
            pool,
            NewOperation {
                account_id,
                date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                value_date: None,
                amount_cents: -500,
                label: label.to_owned(),
                category_id,
                note: None,
                tags: Vec::new(),
            },
        )
        .await
        .unwrap()
        .id
    }

    #[test]
    fn spreads_a_category_to_uncategorized_operations_of_the_label() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let groceries = category_id(&pool, "Courses").await;
            let restaurants = category_id(&pool, "Restaurants").await;
            let checking = new_account(&pool, "Courant").await;
            let card = new_account(&pool, "Carte").await;

            let first = new_operation(&pool, checking, "CB CARREFOUR 01/09", None).await;
            // Same normalized label, on another account too.
            new_operation(&pool, checking, "CB CARREFOUR 02/09", None).await;
            new_operation(&pool, card, "CB CARREFOUR 03/09", None).await;
            let other = new_operation(&pool, card, "CB CARREFOUR 04/09", Some(restaurants)).await;
            new_operation(&pool, checking, "CB FNAC 05/09", None).await;

            operation::set_category(&pool, first, Some(groceries))
                .await
                .unwrap();
            assert_eq!(
                same_label(&pool, first).await.unwrap(),
                SameLabel {
                    normalized_label: "CARREFOUR".to_owned(),
                    uncategorized: 2,
                    other_categories: vec![CategoryCount {
                        category_id: restaurants,
                        count: 1,
                    }],
                }
            );

            assert_eq!(categorize_same_label(&pool, first).await.unwrap(), 2);
            let after = same_label(&pool, first).await.unwrap();
            assert_eq!(after.uncategorized, 0);
            // The operation in another category is left as it is.
            assert_eq!(
                operation::get(&pool, other).await.unwrap().category_id,
                Some(restaurants)
            );
            assert_eq!(categorize_same_label(&pool, first).await.unwrap(), 0);
        });
    }

    #[test]
    fn derives_the_internal_transfer_flag() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let transfer = category_id(&pool, "Virement interne").await;
            let checking = new_account(&pool, "Courant").await;
            let first = new_operation(&pool, checking, "VIR VERS LIVRET A", Some(transfer)).await;
            let second = new_operation(&pool, checking, "VIR VERS LIVRET A", None).await;

            assert_eq!(categorize_same_label(&pool, first).await.unwrap(), 1);
            let second = operation::get(&pool, second).await.unwrap();
            assert_eq!(second.category_id, Some(transfer));
            assert!(second.is_internal_transfer);
        });
    }

    #[test]
    fn an_uncategorized_operation_spreads_nothing() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let checking = new_account(&pool, "Courant").await;
            let first = new_operation(&pool, checking, "CB FNAC", None).await;
            new_operation(&pool, checking, "CB FNAC", None).await;

            assert_eq!(same_label(&pool, first).await.unwrap().uncategorized, 1);
            assert_eq!(categorize_same_label(&pool, first).await.unwrap(), 0);
        });
    }
}
