use std::collections::HashMap;

use sqlx::{SqliteConnection, SqlitePool};

use super::{Tag, TagError, clean_name};
use crate::error::AppResult;

/// Every tag, by name, with the count of its operations.
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Tag>> {
    let tags = sqlx::query_as!(
        Tag,
        r#"SELECT t.id AS "id!", t.name, COUNT(ot.operation_id) AS "operation_count!: i64"
           FROM tag t
           LEFT JOIN operation_tag ot ON ot.tag_id = t.id
           GROUP BY t.id
           ORDER BY t.name COLLATE NOCASE"#
    )
    .fetch_all(pool)
    .await?;
    Ok(tags)
}

/// Renames a tag. A name already used by another tag (ignoring case) is refused.
pub async fn rename(pool: &SqlitePool, id: i64, name: &str) -> AppResult<Tag> {
    let name = clean_name(name).ok_or(TagError::EmptyName)?;
    let mut tx = pool.begin().await?;
    fetch(&mut tx, id).await?;
    let other = sqlx::query_scalar!(
        "SELECT name FROM tag WHERE name = ?1 COLLATE NOCASE AND id <> ?2",
        name,
        id
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(other) = other {
        return Err(TagError::NameTaken(other).into());
    }
    sqlx::query!("UPDATE tag SET name = ?2 WHERE id = ?1", id, name)
        .execute(&mut *tx)
        .await?;
    let tag = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(tag)
}

/// Deletes a tag and removes it from its operations.
pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let deleted = sqlx::query!("DELETE FROM tag WHERE id = ?1", id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(TagError::NotFound(id).into());
    }
    Ok(())
}

/// Puts exactly the tags `names` on an operation. An unknown name creates a tag; a known one,
/// ignoring case, reuses it. Empty names and repeats are skipped.
pub(crate) async fn set_for_operation(
    conn: &mut SqliteConnection,
    operation_id: i64,
    names: &[String],
) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM operation_tag WHERE operation_id = ?1",
        operation_id
    )
    .execute(&mut *conn)
    .await?;
    for name in names.iter().filter_map(|name| clean_name(name)) {
        let existing = sqlx::query_scalar!(
            r#"SELECT id AS "id!" FROM tag WHERE name = ?1 COLLATE NOCASE"#,
            name
        )
        .fetch_optional(&mut *conn)
        .await?;
        let tag_id = match existing {
            Some(id) => id,
            None => sqlx::query!("INSERT INTO tag (name) VALUES (?1)", name)
                .execute(&mut *conn)
                .await?
                .last_insert_rowid(),
        };
        sqlx::query!(
            "INSERT OR IGNORE INTO operation_tag (operation_id, tag_id) VALUES (?1, ?2)",
            operation_id,
            tag_id
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Tag names of an operation, by name.
pub(crate) async fn names_of(
    conn: &mut SqliteConnection,
    operation_id: i64,
) -> AppResult<Vec<String>> {
    let names = sqlx::query_scalar!(
        "SELECT t.name
         FROM operation_tag ot
         JOIN tag t ON t.id = ot.tag_id
         WHERE ot.operation_id = ?1
         ORDER BY t.name COLLATE NOCASE",
        operation_id
    )
    .fetch_all(conn)
    .await?;
    Ok(names)
}

/// Tag names of the operations of an account (of every account with `None`), by operation,
/// each list by name.
pub(crate) async fn names_by_operation(
    pool: &SqlitePool,
    account_id: Option<i64>,
) -> AppResult<HashMap<i64, Vec<String>>> {
    let rows = sqlx::query!(
        "SELECT ot.operation_id, t.name
         FROM operation_tag ot
         JOIN tag t ON t.id = ot.tag_id
         JOIN operation o ON o.id = ot.operation_id
         WHERE ?1 IS NULL OR o.account_id = ?1
         ORDER BY t.name COLLATE NOCASE",
        account_id
    )
    .fetch_all(pool)
    .await?;
    let mut names: HashMap<i64, Vec<String>> = HashMap::new();
    for row in rows {
        names.entry(row.operation_id).or_default().push(row.name);
    }
    Ok(names)
}

async fn fetch(conn: &mut SqliteConnection, id: i64) -> AppResult<Tag> {
    let tag = sqlx::query_as!(
        Tag,
        r#"SELECT t.id AS "id!", t.name, COUNT(ot.operation_id) AS "operation_count!: i64"
           FROM tag t
           LEFT JOIN operation_tag ot ON ot.tag_id = t.id
           WHERE t.id = ?1
           GROUP BY t.id"#,
        id
    )
    .fetch_optional(conn)
    .await?;
    Ok(tag.ok_or(TagError::NotFound(id))?)
}
