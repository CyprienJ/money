//! Categories on two levels, with the system "Virement interne" category.
//!
//! A subcategory inherits the color and the income flag of its top-level category.

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::{SqliteConnection, SqlitePool};

use crate::error::{AppResult, ErrorKind};

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

/// Fields of a category to create or update. For a subcategory, `color` and `is_income` are
/// ignored: they come from the parent.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    pub name: String,
    pub parent_id: Option<i64>,
    pub color: CategoryColor,
    pub is_income: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CategoryError {
    #[error("category {0} not found")]
    NotFound(i64),

    #[error("category name cannot be empty")]
    EmptyName,

    #[error("a category named {0:?} already exists at this place")]
    DuplicateName(String),

    #[error("the internal transfer category cannot be changed or deleted")]
    SystemCategoryLocked,

    #[error("a subcategory must be placed under a top-level category other than internal transfer")]
    InvalidParent,

    #[error("a category with subcategories cannot become a subcategory or be deleted")]
    HasSubcategories,

    #[error("operations cannot be moved to the deleted category itself")]
    ReassignToSelf,
}

impl CategoryError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            CategoryError::NotFound(_) => ErrorKind::NotFound,
            _ => ErrorKind::Validation,
        }
    }
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

pub async fn create(pool: &SqlitePool, input: CategoryInput) -> AppResult<Category> {
    let mut tx = pool.begin().await?;
    let values = check(&mut tx, None, &input).await?;
    let id = sqlx::query!(
        "INSERT INTO category (name, parent_id, color, is_income) VALUES (?1, ?2, ?3, ?4)",
        values.name,
        input.parent_id,
        values.color,
        values.is_income
    )
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    let category = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(category)
}

/// Renames, moves or recolors a category. Subcategories follow the color and income flag of
/// a top-level category.
pub async fn update(pool: &SqlitePool, id: i64, input: CategoryInput) -> AppResult<Category> {
    let mut tx = pool.begin().await?;
    let current = fetch(&mut tx, id).await?;
    if current.is_internal_transfer {
        return Err(CategoryError::SystemCategoryLocked.into());
    }
    if input.parent_id.is_some() && has_children(&mut tx, id).await? {
        return Err(CategoryError::HasSubcategories.into());
    }
    let values = check(&mut tx, Some(id), &input).await?;
    sqlx::query!(
        "UPDATE category SET name = ?2, parent_id = ?3, color = ?4, is_income = ?5 WHERE id = ?1",
        id,
        values.name,
        input.parent_id,
        values.color,
        values.is_income
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE category SET color = ?2, is_income = ?3 WHERE parent_id = ?1",
        id,
        values.color,
        values.is_income
    )
    .execute(&mut *tx)
    .await?;
    let category = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(category)
}

/// Deletes a category without subcategories. Its operations move to `reassign_to`, or become
/// uncategorized with `None`; their internal transfer flag follows the new category.
pub async fn delete(pool: &SqlitePool, id: i64, reassign_to: Option<i64>) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let current = fetch(&mut tx, id).await?;
    if current.is_internal_transfer {
        return Err(CategoryError::SystemCategoryLocked.into());
    }
    if has_children(&mut tx, id).await? {
        return Err(CategoryError::HasSubcategories.into());
    }
    let is_internal_transfer = match reassign_to {
        Some(target) if target == id => return Err(CategoryError::ReassignToSelf.into()),
        Some(target) => fetch(&mut tx, target).await?.is_internal_transfer,
        None => false,
    };

    sqlx::query!(
        "UPDATE operation
         SET is_internal_transfer = ?2
         WHERE id IN (SELECT operation_id FROM operation_split WHERE category_id = ?1)",
        id,
        is_internal_transfer
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE operation_split SET category_id = ?2 WHERE category_id = ?1",
        id,
        reassign_to
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM category WHERE id = ?1", id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn fetch(conn: &mut SqliteConnection, id: i64) -> AppResult<Category> {
    let category = sqlx::query_as!(
        Category,
        r#"SELECT id, name, parent_id, color AS "color: CategoryColor",
                  is_internal_transfer AS "is_internal_transfer: bool", is_income AS "is_income: bool"
           FROM category
           WHERE id = ?1"#,
        id
    )
    .fetch_optional(conn)
    .await?;
    Ok(category.ok_or(CategoryError::NotFound(id))?)
}

async fn has_children(conn: &mut SqliteConnection, id: i64) -> AppResult<bool> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM category WHERE parent_id = ?1"#,
        id
    )
    .fetch_one(conn)
    .await?;
    Ok(count > 0)
}

/// Values to store, once the input is checked.
struct Values {
    name: String,
    color: CategoryColor,
    is_income: bool,
}

/// Checks the name and the parent of `input`, for a new category (`id` = `None`) or an
/// existing one, and resolves the inherited color and income flag.
async fn check(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    input: &CategoryInput,
) -> AppResult<Values> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(CategoryError::EmptyName.into());
    }

    let (color, is_income) = match input.parent_id {
        None => (input.color, input.is_income),
        Some(parent_id) => {
            let parent = match fetch(conn, parent_id).await {
                Err(crate::error::AppError::Category(CategoryError::NotFound(_))) => {
                    return Err(CategoryError::InvalidParent.into());
                }
                result => result?,
            };
            if parent.parent_id.is_some() || parent.is_internal_transfer || Some(parent.id) == id {
                return Err(CategoryError::InvalidParent.into());
            }
            (parent.color, parent.is_income)
        }
    };

    let taken = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64"
           FROM category
           WHERE parent_id IS ?1 AND name = ?2 COLLATE NOCASE AND id IS NOT ?3"#,
        input.parent_id,
        name,
        id
    )
    .fetch_one(conn)
    .await?;
    if taken > 0 {
        return Err(CategoryError::DuplicateName(name.to_owned()).into());
    }

    Ok(Values {
        name: name.to_owned(),
        color,
        is_income,
    })
}

#[cfg(test)]
mod tests;
