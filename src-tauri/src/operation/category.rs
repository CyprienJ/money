use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::error::AppResult;

/// Color family of a category, mapped to the `--cat-<color>` design tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum CategoryColor {
    Housing,
    Groceries,
    Restaurants,
    /// Transport and income.
    Transport,
    Leisure,
    Subscriptions,
    Health,
    /// Internal transfer and other.
    Neutral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    /// `None` for a top-level category. Two levels at most.
    pub parent_id: Option<i64>,
    pub color: CategoryColor,
    /// System category excluded from income and expense statistics.
    pub is_internal_transfer: bool,
    pub is_income: bool,
}

/// Every category, ordered by name. The frontend builds the two-level tree from `parent_id`.
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Category>> {
    let categories = sqlx::query_as!(
        Category,
        r#"SELECT id, name, parent_id, color AS "color: CategoryColor",
                  is_internal_transfer AS "is_internal_transfer: bool", is_income AS "is_income: bool"
           FROM category
           ORDER BY name COLLATE NOCASE"#
    )
    .fetch_all(pool)
    .await?;
    Ok(categories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::memory_pool;

    #[test]
    fn lists_categories_by_name_with_the_system_transfer() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            sqlx::query!(
                "INSERT INTO category (id, name, color, is_income) VALUES (10, 'Revenus', 'transport', 1);
                 INSERT INTO category (name, parent_id, color, is_income) VALUES ('Salaire', 10, 'transport', 1);"
            )
            .execute(&pool)
            .await
            .unwrap();

            let categories = list(&pool).await.unwrap();
            let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
            assert_eq!(names, ["Revenus", "Salaire", "Virement interne"]);

            let salary = &categories[1];
            assert_eq!(salary.parent_id, Some(10));
            assert_eq!(salary.color, CategoryColor::Transport);
            assert!(salary.is_income);
            assert!(categories[2].is_internal_transfer);
            assert_eq!(categories[2].color, CategoryColor::Neutral);
        });
    }
}
