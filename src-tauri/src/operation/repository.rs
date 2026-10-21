use chrono::{NaiveDate, Utc};
use sqlx::{SqliteConnection, SqlitePool};

use super::{NewOperation, Operation, OperationError, OperationRow, OperationUpdate};
use crate::account::{AccountError, AccountType};
use crate::error::AppResult;
use crate::labeling::{memory, normalize_label};
use crate::payee;

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Operation> {
    let mut conn = pool.acquire().await?;
    fetch(&mut conn, id).await
}

/// Operations of an account, most recent first, optionally restricted to a period (bounds
/// included). Each row carries the account balance right after the operation.
pub async fn list(
    pool: &SqlitePool,
    account_id: i64,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> AppResult<Vec<OperationRow>> {
    // The running balance is computed over the whole account history, then filtered.
    // V1 has a single split per operation, so the join does not duplicate rows.
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", account_id AS "account_id!", date AS "date!: NaiveDate",
                  value_date AS "value_date: NaiveDate", amount_cents AS "amount_cents!",
                  raw_label AS "raw_label!", normalized_label AS "normalized_label!",
                  payee_id, payee_name AS "payee_name!: String", import_batch_id, category_id,
                  is_internal_transfer AS "is_internal_transfer!: bool",
                  balance_cents AS "balance_cents!: i64"
           FROM (
               SELECT o.id, o.account_id, o.date, o.value_date, o.amount_cents, o.raw_label,
                      o.normalized_label, o.payee_id,
                      COALESCE(p.name, o.normalized_label) AS payee_name,
                      o.import_batch_id, s.category_id, o.is_internal_transfer,
                      a.opening_balance_cents
                          + SUM(o.amount_cents) OVER (ORDER BY o.date, o.id) AS balance_cents
               FROM operation o
               JOIN account a ON a.id = o.account_id
               LEFT JOIN operation_split s ON s.operation_id = o.id
               LEFT JOIN payee p ON p.id = o.payee_id
               WHERE o.account_id = ?1
           )
           WHERE (?2 IS NULL OR date >= ?2) AND (?3 IS NULL OR date <= ?3)
           ORDER BY date DESC, id DESC"#,
        account_id,
        from,
        to
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| OperationRow {
            operation: Operation {
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
                category_id: row.category_id,
                is_internal_transfer: row.is_internal_transfer,
            },
            balance_cents: row.balance_cents,
        })
        .collect())
}

/// Records a manual entry.
pub async fn create(pool: &SqlitePool, new: NewOperation) -> AppResult<Operation> {
    let label = clean_label(&new.label)?;
    check_amount(new.amount_cents)?;

    let mut tx = pool.begin().await?;
    check_account(&mut tx, new.account_id, new.date).await?;
    let is_internal_transfer = internal_transfer_flag(&mut tx, new.category_id).await?;
    let normalized_label = normalize_label(&label);
    let payee_id = payee::resolve(&mut tx, &normalized_label).await?;
    let now = timestamp();

    let id = sqlx::query!(
        "INSERT INTO operation
         (account_id, date, value_date, amount_cents, raw_label, normalized_label, payee_id,
          is_internal_transfer, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        new.account_id,
        new.date,
        new.value_date,
        new.amount_cents,
        label,
        normalized_label,
        payee_id,
        is_internal_transfer,
        now
    )
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    sqlx::query!(
        "INSERT INTO operation_split (operation_id, category_id, amount_cents) VALUES (?1, ?2, ?3)",
        id,
        new.category_id,
        new.amount_cents
    )
    .execute(&mut *tx)
    .await?;
    memory::remember(&mut tx, &normalized_label, new.category_id, &now).await?;

    let operation = fetch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(operation)
}

/// Updates an operation. The date, amount and label of an imported operation are locked.
pub async fn update(pool: &SqlitePool, id: i64, changes: OperationUpdate) -> AppResult<Operation> {
    let mut tx = pool.begin().await?;
    let current = fetch(&mut tx, id).await?;
    let now = timestamp();

    if current.import_batch_id.is_some() {
        if changes.date != current.date
            || changes.amount_cents != current.amount_cents
            || changes.label != current.raw_label
        {
            return Err(OperationError::ImportedFieldsLocked.into());
        }
        sqlx::query!(
            "UPDATE operation SET value_date = ?2, updated_at = ?3 WHERE id = ?1",
            id,
            changes.value_date,
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
                 normalized_label = ?6, payee_id = ?7, updated_at = ?8
             WHERE id = ?1",
            id,
            changes.date,
            changes.value_date,
            changes.amount_cents,
            label,
            normalized_label,
            payee_id,
            now
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE operation_split SET amount_cents = ?2 WHERE operation_id = ?1",
            id,
            changes.amount_cents
        )
        .execute(&mut *tx)
        .await?;
    }

    write_category(&mut tx, id, changes.category_id, &now).await?;
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
    let operation = sqlx::query_as!(
        Operation,
        r#"SELECT o.id, o.account_id, o.date AS "date: NaiveDate",
                  o.value_date AS "value_date: NaiveDate", o.amount_cents, o.raw_label,
                  o.normalized_label, o.payee_id,
                  COALESCE(p.name, o.normalized_label) AS "payee_name!: String",
                  o.import_batch_id, s.category_id,
                  o.is_internal_transfer AS "is_internal_transfer: bool"
           FROM operation o
           LEFT JOIN operation_split s ON s.operation_id = o.id
           LEFT JOIN payee p ON p.id = o.payee_id
           WHERE o.id = ?1"#,
        id
    )
    .fetch_optional(conn)
    .await?;
    Ok(operation.ok_or(OperationError::NotFound(id))?)
}

/// Writes the category on the split and the derived internal transfer flag on the operation,
/// and remembers it for the operation label: this is a categorization by the user.
async fn write_category(
    conn: &mut SqliteConnection,
    id: i64,
    category_id: Option<i64>,
    now: &str,
) -> AppResult<()> {
    let is_internal_transfer = internal_transfer_flag(conn, category_id).await?;
    sqlx::query!(
        "UPDATE operation_split SET category_id = ?2 WHERE operation_id = ?1",
        id,
        category_id
    )
    .execute(&mut *conn)
    .await?;
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
