//! Categorization rules: "normalized label contains X" gives a category. The first rule in the
//! user's order that matches wins; rules come before the category memory at import.

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::{SqliteConnection, SqlitePool};

use crate::error::{AppResult, ErrorKind};
use crate::operation::{internal_transfer_flag, timestamp};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: i64,
    /// Text looked for in the normalized label, ignoring case and accents.
    pub pattern: String,
    pub category_id: i64,
    /// Operations still to categorize that the rule matches.
    pub pending_count: u32,
}

/// Fields of a rule to create or update.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub pattern: String,
    pub category_id: i64,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RuleError {
    #[error("rule {0} not found")]
    NotFound(i64),

    #[error("rule text cannot be empty")]
    EmptyPattern,

    #[error("category {0} not found")]
    CategoryNotFound(i64),

    #[error("the new order must list every rule once")]
    InvalidOrder,
}

impl RuleError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            RuleError::NotFound(_) | RuleError::CategoryNotFound(_) => ErrorKind::NotFound,
            _ => ErrorKind::Validation,
        }
    }
}

/// Rules ready to match labels, in their order.
pub struct Rules(Vec<(String, i64, String)>);

impl Rules {
    pub async fn load(conn: &mut SqliteConnection) -> AppResult<Self> {
        let rows = sqlx::query!(
            "SELECT pattern, category_id FROM categorization_rule ORDER BY position, id"
        )
        .fetch_all(conn)
        .await?;
        Ok(Rules(
            rows.into_iter()
                .map(|row| (fold(&row.pattern), row.category_id, row.pattern))
                .collect(),
        ))
    }

    /// Category and pattern of the first rule matching `normalized_label`.
    pub fn find(&self, normalized_label: &str) -> Option<(i64, &str)> {
        let label = fold(normalized_label);
        self.0
            .iter()
            .find(|(folded, _, _)| label.contains(folded.as_str()))
            .map(|(_, category_id, pattern)| (*category_id, pattern.as_str()))
    }
}

/// Every rule, in order, with the count of operations to categorize it matches.
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Rule>> {
    let rows = sqlx::query!(
        "SELECT id, pattern, category_id FROM categorization_rule ORDER BY position, id"
    )
    .fetch_all(pool)
    .await?;
    let pending = pending_labels(pool).await?;
    Ok(rows
        .into_iter()
        .map(|row| Rule {
            pending_count: count_matches(&pending, &row.pattern),
            id: row.id,
            pattern: row.pattern,
            category_id: row.category_id,
        })
        .collect())
}

/// Operations still to categorize whose label contains `pattern`, to preview a rule.
pub async fn count_pending(pool: &SqlitePool, pattern: &str) -> AppResult<u32> {
    let pattern = clean_pattern(pattern)?;
    Ok(count_matches(&pending_labels(pool).await?, &pattern))
}

/// Adds a rule at the end of the list.
pub async fn create(pool: &SqlitePool, input: RuleInput) -> AppResult<Rule> {
    let pattern = clean_pattern(&input.pattern)?;
    let mut tx = pool.begin().await?;
    check_category(&mut tx, input.category_id).await?;
    let id = sqlx::query!(
        "INSERT INTO categorization_rule (pattern, category_id, position)
         VALUES (?1, ?2, (SELECT COALESCE(MAX(position), -1) + 1 FROM categorization_rule))",
        pattern,
        input.category_id
    )
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    tx.commit().await?;
    get(pool, id).await
}

pub async fn update(pool: &SqlitePool, id: i64, input: RuleInput) -> AppResult<Rule> {
    let pattern = clean_pattern(&input.pattern)?;
    let mut tx = pool.begin().await?;
    check_category(&mut tx, input.category_id).await?;
    let updated = sqlx::query!(
        "UPDATE categorization_rule SET pattern = ?2, category_id = ?3 WHERE id = ?1",
        id,
        pattern,
        input.category_id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(RuleError::NotFound(id).into());
    }
    tx.commit().await?;
    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let deleted = sqlx::query!("DELETE FROM categorization_rule WHERE id = ?1", id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(RuleError::NotFound(id).into());
    }
    Ok(())
}

/// Puts the rules in the order of `ids`, which lists every rule once.
pub async fn reorder(pool: &SqlitePool, ids: &[i64]) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let mut current = sqlx::query_scalar!("SELECT id FROM categorization_rule")
        .fetch_all(&mut *tx)
        .await?;
    let mut wanted = ids.to_vec();
    current.sort_unstable();
    wanted.sort_unstable();
    if current != wanted {
        return Err(RuleError::InvalidOrder.into());
    }
    for (position, id) in (0_i64..).zip(ids) {
        sqlx::query!(
            "UPDATE categorization_rule SET position = ?2 WHERE id = ?1",
            id,
            position
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Gives the category of rule `id` to the operations still to categorize that it matches;
/// returns their count. Categorized operations are left as they are, and the category memory
/// is not changed: this is not a choice made on a label.
pub async fn apply(pool: &SqlitePool, id: i64) -> AppResult<u32> {
    let rule = get(pool, id).await?;
    let folded = fold(&rule.pattern);
    let mut tx = pool.begin().await?;
    let is_internal_transfer = internal_transfer_flag(&mut tx, Some(rule.category_id)).await?;
    let now = timestamp();
    let pending = sqlx::query!(
        r#"SELECT o.id AS "id!", o.normalized_label
           FROM operation o
           JOIN operation_split s ON s.operation_id = o.id
           WHERE s.category_id IS NULL"#
    )
    .fetch_all(&mut *tx)
    .await?;

    let mut applied = 0;
    for operation in pending
        .iter()
        .filter(|o| fold(&o.normalized_label).contains(&folded))
    {
        sqlx::query!(
            "UPDATE operation_split SET category_id = ?2 WHERE operation_id = ?1",
            operation.id,
            rule.category_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE operation SET is_internal_transfer = ?2, updated_at = ?3 WHERE id = ?1",
            operation.id,
            is_internal_transfer,
            now
        )
        .execute(&mut *tx)
        .await?;
        applied += 1;
    }
    tx.commit().await?;
    Ok(applied)
}

async fn get(pool: &SqlitePool, id: i64) -> AppResult<Rule> {
    list(pool)
        .await?
        .into_iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| RuleError::NotFound(id).into())
}

async fn check_category(conn: &mut SqliteConnection, category_id: i64) -> AppResult<()> {
    let exists = sqlx::query_scalar!("SELECT id FROM category WHERE id = ?1", category_id)
        .fetch_optional(conn)
        .await?;
    if exists.is_none() {
        return Err(RuleError::CategoryNotFound(category_id).into());
    }
    Ok(())
}

/// Normalized labels of the operations still to categorize, folded.
async fn pending_labels(pool: &SqlitePool) -> AppResult<Vec<String>> {
    let labels = sqlx::query_scalar!(
        "SELECT o.normalized_label
         FROM operation o
         JOIN operation_split s ON s.operation_id = o.id
         WHERE s.category_id IS NULL"
    )
    .fetch_all(pool)
    .await?;
    Ok(labels.iter().map(|label| fold(label)).collect())
}

fn count_matches(folded_labels: &[String], pattern: &str) -> u32 {
    let pattern = fold(pattern);
    let count = folded_labels
        .iter()
        .filter(|label| label.contains(&pattern))
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn clean_pattern(pattern: &str) -> Result<String, RuleError> {
    let pattern = pattern.split_whitespace().collect::<Vec<_>>().join(" ");
    if pattern.is_empty() {
        return Err(RuleError::EmptyPattern);
    }
    Ok(pattern)
}

/// Uppercase text without accents nor repeated spaces, to compare a pattern with a label.
fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.to_uppercase().chars() {
        match c {
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => folded.push('A'),
            'Ç' => folded.push('C'),
            'È' | 'É' | 'Ê' | 'Ë' => folded.push('E'),
            'Ì' | 'Í' | 'Î' | 'Ï' => folded.push('I'),
            'Ñ' => folded.push('N'),
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => folded.push('O'),
            'Ù' | 'Ú' | 'Û' | 'Ü' => folded.push('U'),
            'Ý' | 'Ÿ' => folded.push('Y'),
            'Æ' => folded.push_str("AE"),
            'Œ' => folded.push_str("OE"),
            _ => folded.push(c),
        }
    }
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;
