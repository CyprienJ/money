use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use sqlx::{SqliteConnection, SqlitePool};

use super::transfer;
use super::{NewOperation, Operation, OperationError, OperationUpdate, SplitLine};
use crate::account::{AccountError, AccountType};
use crate::error::AppResult;
use crate::labeling::{memory, normalize_label};
use crate::payee;
use crate::tag;

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Operation> {
    let mut conn = pool.acquire().await?;
    fetch(&mut conn, id).await
}

/// Records a manual entry.
pub async fn create(pool: &SqlitePool, new: NewOperation) -> AppResult<Operation> {
    let label = clean_label(&new.label)?;
    check_amount(new.amount_cents)?;
    let note = clean_note(new.note.as_deref());

    let mut tx = pool.begin().await?;
    check_account(&mut tx, new.account_id, new.date).await?;
    let category_id = if new.splits.is_empty() {
        new.category_id
    } else {
        check_splits(&mut tx, new.amount_cents, &new.splits).await?;
        None
    };
    let is_internal_transfer = internal_transfer_flag(&mut tx, category_id).await?;
    let normalized_label = normalize_label(&label);
    let payee_id = payee::resolve(&mut tx, &normalized_label).await?;
    let now = timestamp();

    let id = sqlx::query!(
        "INSERT INTO operation
         (account_id, date, value_date, amount_cents, raw_label, normalized_label, payee_id,
          is_internal_transfer, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        new.account_id,
        new.date,
        new.value_date,
        new.amount_cents,
        label,
        normalized_label,
        payee_id,
        is_internal_transfer,
        note,
        now
    )
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    if new.splits.is_empty() {
        write_single_split(&mut tx, id, category_id).await?;
        memory::remember(&mut tx, &normalized_label, category_id, &now).await?;
    } else {
        // A split is not remembered for the label: it is particular to the operation.
        write_parts(&mut tx, id, &new.splits).await?;
    }
    tag::set_for_operation(&mut tx, id, &new.tags).await?;

    let operation = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(operation)
}

/// Updates an operation. The date, amount and label of an imported operation are locked.
pub async fn update(pool: &SqlitePool, id: i64, changes: OperationUpdate) -> AppResult<Operation> {
    let mut tx = pool.begin().await?;
    let current = fetch(&mut tx, id).await?;
    let now = timestamp();
    let note = clean_note(changes.note.as_deref());

    if current.import_batch_id.is_some() {
        if changes.date != current.date
            || changes.amount_cents != current.amount_cents
            || changes.label != current.raw_label
        {
            return Err(OperationError::ImportedFieldsLocked.into());
        }
        sqlx::query!(
            "UPDATE operation SET value_date = ?2, note = ?3, updated_at = ?4 WHERE id = ?1",
            id,
            changes.value_date,
            note,
            now
        )
        .execute(&mut *tx)
        .await?;
    } else {
        let label = clean_label(&changes.label)?;
        check_amount(changes.amount_cents)?;
        check_account(&mut tx, current.account_id, changes.date).await?;
        let normalized_label = normalize_label(&label);
        let payee_id = payee::resolve(&mut tx, &normalized_label).await?;
        sqlx::query!(
            "UPDATE operation
             SET date = ?2, value_date = ?3, amount_cents = ?4, raw_label = ?5,
                 normalized_label = ?6, payee_id = ?7, note = ?8, updated_at = ?9
             WHERE id = ?1",
            id,
            changes.date,
            changes.value_date,
            changes.amount_cents,
            label,
            normalized_label,
            payee_id,
            note,
            now
        )
        .execute(&mut *tx)
        .await?;
    }

    if !changes.splits.is_empty() {
        check_splits(&mut tx, changes.amount_cents, &changes.splits).await?;
        if changes.splits != current.splits {
            transfer::unlink_conn(&mut tx, id).await?;
            write_parts(&mut tx, id, &changes.splits).await?;
            sqlx::query!(
                "UPDATE operation SET is_internal_transfer = 0, updated_at = ?2 WHERE id = ?1",
                id,
                now
            )
            .execute(&mut *tx)
            .await?;
        }
    } else if changes.category_id != current.category_id || !current.splits.is_empty() {
        // Only a new category is a categorization by the user, remembered for the label.
        write_category(&mut tx, id, changes.category_id, &now).await?;
    } else {
        // The single split follows the amount of the operation.
        write_single_split(&mut tx, id, current.category_id).await?;
    }
    tag::set_for_operation(&mut tx, id, &changes.tags).await?;
    let operation = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(operation)
}

/// Sets (or clears, with `None`) the category of an operation, imported or not.
pub async fn set_category(
    pool: &SqlitePool,
    id: i64,
    category_id: Option<i64>,
) -> AppResult<Operation> {
    let mut tx = pool.begin().await?;
    fetch(&mut tx, id).await?;
    write_category(&mut tx, id, category_id, &timestamp()).await?;
    let operation = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(operation)
}

/// Deletes a manual entry. Imported operations are removed by cancelling their import.
pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    if fetch(&mut tx, id).await?.import_batch_id.is_some() {
        return Err(OperationError::ImportedOperationNotDeletable.into());
    }
    sqlx::query!("DELETE FROM operation WHERE id = ?1", id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn fetch(conn: &mut SqliteConnection, id: i64) -> AppResult<Operation> {
    let row = sqlx::query!(
        r#"SELECT o.id, o.account_id, o.date AS "date: NaiveDate",
                  o.value_date AS "value_date: NaiveDate", o.amount_cents, o.raw_label,
                  o.normalized_label, o.payee_id,
                  COALESCE(p.name, o.normalized_label) AS "payee_name!: String",
                  o.import_batch_id,
                  COALESCE(b.stored_file IS NOT NULL, 0) AS "has_source_document!: bool",
                  o.source_page AS "source_page: u32", o.source_line AS "source_line: u32",
                  o.is_internal_transfer AS "is_internal_transfer: bool", o.transfer_pair_id,
                  o.note,
                  (SELECT COUNT(*) FROM attachment a WHERE a.operation_id = o.id)
                      AS "attachment_count!: i64"
           FROM operation o
           LEFT JOIN payee p ON p.id = o.payee_id
           LEFT JOIN import_batch b ON b.id = o.import_batch_id
           WHERE o.id = ?1"#,
        id
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(OperationError::NotFound(id))?;
    let mut splits = sqlx::query!(
        "SELECT category_id, amount_cents FROM operation_split WHERE operation_id = ?1 ORDER BY id",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    let (category_id, splits) = if splits.len() == 1 {
        (splits.remove(0).category_id, Vec::new())
    } else {
        let parts = splits
            .into_iter()
            .filter_map(|s| {
                Some(SplitLine {
                    category_id: s.category_id?,
                    amount_cents: s.amount_cents,
                })
            })
            .collect();
        (None, parts)
    };
    Ok(Operation {
        id: row.id,
        account_id: row.account_id,
        date: row.date,
        value_date: row.value_date,
        amount_cents: row.amount_cents,
        raw_label: row.raw_label,
        normalized_label: row.normalized_label,
        payee_id: row.payee_id,
        payee_name: row.payee_name,
        import_batch_id: row.import_batch_id,
        has_source_document: row.has_source_document,
        source_page: row.source_page,
        source_line: row.source_line,
        category_id,
        splits,
        is_internal_transfer: row.is_internal_transfer,
        transfer_pair_id: row.transfer_pair_id,
        note: row.note,
        tags: tag::names_of(conn, id).await?,
        attachment_count: row.attachment_count,
    })
}

/// Writes the category on the single split (replacing the parts of a split operation) and the derived internal transfer flag on the operation,
/// and remembers it for the operation label: this is a categorization by the user. An operation
/// leaving the internal transfer category is unlinked from its transfer.
async fn write_category(
    conn: &mut SqliteConnection,
    id: i64,
    category_id: Option<i64>,
    now: &str,
) -> AppResult<()> {
    let is_internal_transfer = internal_transfer_flag(conn, category_id).await?;
    if !is_internal_transfer {
        transfer::unlink_conn(conn, id).await?;
    }
    write_single_split(conn, id, category_id).await?;
    sqlx::query!(
        "UPDATE operation SET is_internal_transfer = ?2, updated_at = ?3 WHERE id = ?1",
        id,
        is_internal_transfer,
        now
    )
    .execute(&mut *conn)
    .await?;
    let normalized_label =
        sqlx::query_scalar!("SELECT normalized_label FROM operation WHERE id = ?1", id)
            .fetch_one(&mut *conn)
            .await?;
    memory::remember(conn, &normalized_label, category_id, now).await?;
    Ok(())
}

/// Parts of the split operations of an account (of every account with `None`), by operation.
pub(super) async fn parts_by_operation(
    pool: &SqlitePool,
    account_id: Option<i64>,
) -> AppResult<HashMap<i64, Vec<SplitLine>>> {
    let rows = sqlx::query!(
        r#"SELECT s.operation_id, s.category_id AS "category_id!", s.amount_cents
           FROM operation_split s
           JOIN operation o ON o.id = s.operation_id
           WHERE (?1 IS NULL OR o.account_id = ?1)
             AND s.operation_id IN
                 (SELECT operation_id FROM operation_split GROUP BY operation_id HAVING COUNT(*) > 1)
           ORDER BY s.operation_id, s.id"#,
        account_id
    )
    .fetch_all(pool)
    .await?;
    let mut parts: HashMap<i64, Vec<SplitLine>> = HashMap::new();
    for row in rows {
        parts.entry(row.operation_id).or_default().push(SplitLine {
            category_id: row.category_id,
            amount_cents: row.amount_cents,
        });
    }
    Ok(parts)
}

/// Replaces the splits of an operation by one split of its whole amount in `category_id`.
pub(super) async fn write_single_split(
    conn: &mut SqliteConnection,
    id: i64,
    category_id: Option<i64>,
) -> AppResult<()> {
    sqlx::query!("DELETE FROM operation_split WHERE operation_id = ?1", id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "INSERT INTO operation_split (operation_id, category_id, amount_cents)
         SELECT id, ?2, amount_cents FROM operation WHERE id = ?1",
        id,
        category_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Replaces the splits of an operation by `parts`, already checked.
async fn write_parts(conn: &mut SqliteConnection, id: i64, parts: &[SplitLine]) -> AppResult<()> {
    sqlx::query!("DELETE FROM operation_split WHERE operation_id = ?1", id)
        .execute(&mut *conn)
        .await?;
    for part in parts {
        sqlx::query!(
            "INSERT INTO operation_split (operation_id, category_id, amount_cents) VALUES (?1, ?2, ?3)",
            id,
            part.category_id,
            part.amount_cents
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Checks the parts of a split operation of `amount_cents`: at least two, existing categories
/// other than the internal transfer one, non-zero amounts of the operation's sign, summing to it.
async fn check_splits(
    conn: &mut SqliteConnection,
    amount_cents: i64,
    parts: &[SplitLine],
) -> AppResult<()> {
    if parts.len() < 2 {
        return Err(OperationError::SplitTooFewParts.into());
    }
    for part in parts {
        if part.amount_cents == 0 || part.amount_cents.signum() != amount_cents.signum() {
            return Err(OperationError::SplitPartAmount.into());
        }
        if internal_transfer_flag(conn, Some(part.category_id)).await? {
            return Err(OperationError::SplitInternalTransfer.into());
        }
    }
    let actual = parts
        .iter()
        .try_fold(0_i64, |sum, part| sum.checked_add(part.amount_cents))
        .ok_or(OperationError::SplitPartAmount)?;
    if actual != amount_cents {
        return Err(OperationError::SplitTotalMismatch {
            expected: amount_cents,
            actual,
        }
        .into());
    }
    Ok(())
}

/// Checks that the account takes operations and that `date` is not before its opening.
pub(crate) async fn check_account(
    conn: &mut SqliteConnection,
    account_id: i64,
    date: NaiveDate,
) -> AppResult<()> {
    let account = sqlx::query!(
        r#"SELECT type AS "account_type: AccountType", opening_date AS "opening_date: NaiveDate"
           FROM account WHERE id = ?1"#,
        account_id
    )
    .fetch_optional(conn)
    .await?
    .ok_or(AccountError::NotFound(account_id))?;

    if !account.account_type.has_operations() {
        return Err(OperationError::AccountWithoutOperations.into());
    }
    if date < account.opening_date {
        return Err(OperationError::DateBeforeOpening(account.opening_date).into());
    }
    Ok(())
}

/// Whether the category is the internal transfer one (`false` when uncategorized).
pub(crate) async fn internal_transfer_flag(
    conn: &mut SqliteConnection,
    category_id: Option<i64>,
) -> AppResult<bool> {
    let Some(category_id) = category_id else {
        return Ok(false);
    };
    let flag = sqlx::query_scalar!(
        r#"SELECT is_internal_transfer AS "flag: bool" FROM category WHERE id = ?1"#,
        category_id
    )
    .fetch_optional(conn)
    .await?;
    Ok(flag.ok_or(OperationError::CategoryNotFound(category_id))?)
}

fn clean_label(label: &str) -> Result<String, OperationError> {
    let label = label.trim();
    if label.is_empty() {
        return Err(OperationError::EmptyLabel);
    }
    Ok(label.to_owned())
}

/// Note without surrounding spaces; `None` when empty.
fn clean_note(note: Option<&str>) -> Option<String> {
    note.map(str::trim)
        .filter(|note| !note.is_empty())
        .map(str::to_owned)
}

fn check_amount(amount_cents: i64) -> Result<(), OperationError> {
    if amount_cents == 0 {
        return Err(OperationError::ZeroAmount);
    }
    Ok(())
}

/// Current UTC time in the storage format `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn timestamp() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
