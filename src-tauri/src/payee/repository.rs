use std::collections::HashMap;

use chrono::NaiveDate;
use sqlx::{SqliteConnection, SqlitePool};

use super::{Payee, PayeeError, default_name};
use crate::error::AppResult;

/// Payees that have operations, by name, with the labels of their operations.
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Payee>> {
    let mut conn = pool.acquire().await?;
    summaries(&mut conn, None).await
}

/// Renames a payee. A name already used by another payee (ignoring case) is refused: the
/// frontend then offers to merge both payees.
pub async fn rename(pool: &SqlitePool, id: i64, name: &str) -> AppResult<Payee> {
    let name = name.trim();
    if name.is_empty() {
        return Err(PayeeError::EmptyName.into());
    }
    let mut tx = pool.begin().await?;
    check_exists(&mut tx, id).await?;
    let other = sqlx::query!(
        r#"SELECT id AS "id!", name FROM payee WHERE name = ?1 COLLATE NOCASE AND id <> ?2"#,
        name,
        id
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(other) = other {
        return Err(PayeeError::NameTaken {
            name: other.name,
            payee_id: other.id,
        }
        .into());
    }
    sqlx::query!("UPDATE payee SET name = ?2 WHERE id = ?1", id, name)
        .execute(&mut *tx)
        .await?;
    let payee = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(payee)
}

/// Merges the `sources` payees into `target`, which keeps its name: their labels and
/// operations move to `target`, and they are deleted.
pub async fn merge(pool: &SqlitePool, target: i64, sources: &[i64]) -> AppResult<Payee> {
    if sources.contains(&target) {
        return Err(PayeeError::MergeIntoItself.into());
    }
    let mut tx = pool.begin().await?;
    check_exists(&mut tx, target).await?;
    for &source in sources {
        check_exists(&mut tx, source).await?;
        sqlx::query!(
            "UPDATE payee_alias SET payee_id = ?2 WHERE payee_id = ?1",
            source,
            target
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE operation SET payee_id = ?2 WHERE payee_id = ?1",
            source,
            target
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM payee WHERE id = ?1", source)
            .execute(&mut *tx)
            .await?;
    }
    let payee = fetch(&mut tx, target).await?;
    tx.commit().await?;
    Ok(payee)
}

/// Detaches a label from its merged payee into a new payee named after it; returns the new
/// payee. The last label of a payee cannot be detached.
pub async fn detach(pool: &SqlitePool, normalized_label: &str) -> AppResult<Payee> {
    let mut tx = pool.begin().await?;
    let current = sqlx::query_scalar!(
        "SELECT payee_id FROM payee_alias WHERE normalized_label = ?1",
        normalized_label
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| PayeeError::LabelNotFound(normalized_label.to_owned()))?;
    let other_labels = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT normalized_label) AS "count!: i64" FROM operation
           WHERE payee_id = ?1 AND normalized_label <> ?2"#,
        current,
        normalized_label
    )
    .fetch_one(&mut *tx)
    .await?;
    if other_labels == 0 {
        return Err(PayeeError::LastLabel.into());
    }

    let name = free_name(&mut tx, &default_name(normalized_label)).await?;
    let id = insert(&mut tx, &name).await?;
    sqlx::query!(
        "UPDATE payee_alias SET payee_id = ?2 WHERE normalized_label = ?1",
        normalized_label,
        id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE operation SET payee_id = ?2 WHERE normalized_label = ?1",
        normalized_label,
        id
    )
    .execute(&mut *tx)
    .await?;
    let payee = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(payee)
}

/// Gives a payee to the operations that have none: existing operations when payees are
/// introduced, and any operation written without one. Returns their count.
pub async fn assign_missing(pool: &SqlitePool) -> AppResult<u64> {
    let mut tx = pool.begin().await?;
    let labels = sqlx::query_scalar!(
        "SELECT DISTINCT normalized_label FROM operation WHERE payee_id IS NULL"
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut assigned = 0;
    for label in labels {
        let payee_id = resolve(&mut tx, &label).await?;
        assigned += sqlx::query!(
            "UPDATE operation SET payee_id = ?2 WHERE normalized_label = ?1 AND payee_id IS NULL",
            label,
            payee_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
    }
    tx.commit().await?;
    Ok(assigned)
}

/// Payee of a normalized label, created when the label is new: it joins the payee already
/// named like the label's default name, or else gets a new payee of that name.
pub(crate) async fn resolve(conn: &mut SqliteConnection, normalized_label: &str) -> AppResult<i64> {
    let alias = sqlx::query_scalar!(
        "SELECT payee_id FROM payee_alias WHERE normalized_label = ?1",
        normalized_label
    )
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(payee_id) = alias {
        return Ok(payee_id);
    }

    let name = default_name(normalized_label);
    let named = sqlx::query_scalar!(
        r#"SELECT id AS "id!" FROM payee WHERE name = ?1 COLLATE NOCASE"#,
        name
    )
    .fetch_optional(&mut *conn)
    .await?;
    let payee_id = match named {
        Some(id) => id,
        None => insert(conn, &name).await?,
    };
    sqlx::query!(
        "INSERT INTO payee_alias (normalized_label, payee_id) VALUES (?1, ?2)",
        normalized_label,
        payee_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(payee_id)
}

/// Payee of an operation whose normalized label was recomputed: the payee of the new label
/// when it has one, or else the operation keeps `payee_id`, which takes the new label.
pub(crate) async fn follow_label(
    conn: &mut SqliteConnection,
    payee_id: Option<i64>,
    new_label: &str,
) -> AppResult<Option<i64>> {
    if let Some(payee_id) = payee_id {
        sqlx::query!(
            "INSERT INTO payee_alias (normalized_label, payee_id) VALUES (?1, ?2)
             ON CONFLICT (normalized_label) DO NOTHING",
            new_label,
            payee_id
        )
        .execute(&mut *conn)
        .await?;
    }
    let payee_id = sqlx::query_scalar!(
        "SELECT payee_id FROM payee_alias WHERE normalized_label = ?1",
        new_label
    )
    .fetch_optional(conn)
    .await?;
    Ok(payee_id)
}

/// Payee name of each label among `normalized_labels`: the name of its payee, or the default
/// name it will get when imported.
pub(crate) async fn names_of(
    conn: &mut SqliteConnection,
    normalized_labels: &[&str],
) -> AppResult<HashMap<String, String>> {
    let mut names = HashMap::new();
    for &label in normalized_labels {
        if names.contains_key(label) {
            continue;
        }
        let name = sqlx::query_scalar!(
            "SELECT p.name FROM payee_alias a JOIN payee p ON p.id = a.payee_id
             WHERE a.normalized_label = ?1",
            label
        )
        .fetch_optional(&mut *conn)
        .await?;
        names.insert(
            label.to_owned(),
            name.unwrap_or_else(|| default_name(label)),
        );
    }
    Ok(names)
}

async fn insert(conn: &mut SqliteConnection, name: &str) -> AppResult<i64> {
    let id = sqlx::query!("INSERT INTO payee (name) VALUES (?1)", name)
        .execute(conn)
        .await?
        .last_insert_rowid();
    Ok(id)
}

/// `name`, or the first of `name (2)`, `name (3)`… that no payee uses.
async fn free_name(conn: &mut SqliteConnection, name: &str) -> AppResult<String> {
    let mut candidate = name.to_owned();
    for number in 2.. {
        let taken = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM payee WHERE name = ?1 COLLATE NOCASE) AS "taken!: bool""#,
            candidate
        )
        .fetch_one(&mut *conn)
        .await?;
        if !taken {
            break;
        }
        candidate = format!("{name} ({number})");
    }
    Ok(candidate)
}

async fn check_exists(conn: &mut SqliteConnection, id: i64) -> AppResult<()> {
    sqlx::query_scalar!(r#"SELECT id AS "id!" FROM payee WHERE id = ?1"#, id)
        .fetch_optional(conn)
        .await?
        .ok_or(PayeeError::NotFound(id))?;
    Ok(())
}

async fn fetch(conn: &mut SqliteConnection, id: i64) -> AppResult<Payee> {
    let payee = summaries(conn, Some(id)).await?.pop();
    Ok(payee.ok_or(PayeeError::NotFound(id))?)
}

/// Payee `id` even without operations, or every payee that has operations.
async fn summaries(conn: &mut SqliteConnection, id: Option<i64>) -> AppResult<Vec<Payee>> {
    let rows = sqlx::query!(
        r#"SELECT p.id AS "id!", p.name AS "name!",
                  COUNT(o.id) AS "operation_count!: i64",
                  SUM(CASE WHEN o.is_internal_transfer = 0 THEN o.amount_cents END)
                      AS "total_cents: i64",
                  MAX(o.date) AS "last_date: NaiveDate"
           FROM payee p
           LEFT JOIN operation o ON o.payee_id = p.id
           WHERE ?1 IS NULL OR p.id = ?1
           GROUP BY p.id
           HAVING ?1 IS NOT NULL OR COUNT(o.id) > 0
           ORDER BY p.name COLLATE NOCASE"#,
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    let labels = sqlx::query!(
        r#"SELECT DISTINCT payee_id AS "payee_id!", normalized_label FROM operation
           WHERE payee_id IS NOT NULL AND (?1 IS NULL OR payee_id = ?1)
           ORDER BY normalized_label"#,
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut labels_by_payee: HashMap<i64, Vec<String>> = HashMap::new();
    for row in labels {
        labels_by_payee
            .entry(row.payee_id)
            .or_default()
            .push(row.normalized_label);
    }

    Ok(rows
        .into_iter()
        .map(|row| Payee {
            labels: labels_by_payee.remove(&row.id).unwrap_or_default(),
            id: row.id,
            name: row.name,
            operation_count: row.operation_count,
            total_cents: row.total_cents,
            last_date: row.last_date,
        })
        .collect())
}
