//! Valuations of investment, asset and liability accounts: their value at a date, or the
//! outstanding capital of a debt (stored as a positive amount, subtracted in net worth).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::{AccountError, get};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Valuation {
    pub id: i64,
    pub account_id: i64,
    pub date: NaiveDate,
    /// Value, or outstanding capital for a debt: never negative.
    pub value_cents: i64,
}

/// Date and value of a valuation to record or change.
#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ValuationInput {
    pub date: NaiveDate,
    pub value_cents: i64,
}

/// Valuations of an account, the most recent first.
pub async fn list(pool: &SqlitePool, account_id: i64) -> AppResult<Vec<Valuation>> {
    get(pool, account_id).await?;
    let valuations = sqlx::query_as!(
        Valuation,
        r#"SELECT id AS "id!", account_id, date AS "date: NaiveDate", value_cents
           FROM valuation
           WHERE account_id = ?1
           ORDER BY date DESC"#,
        account_id
    )
    .fetch_all(pool)
    .await?;
    Ok(valuations)
}

pub async fn create(
    pool: &SqlitePool,
    account_id: i64,
    input: ValuationInput,
) -> AppResult<Valuation> {
    check(pool, account_id, input).await?;
    let id = sqlx::query!(
        "INSERT INTO valuation (account_id, date, value_cents) VALUES (?1, ?2, ?3)",
        account_id,
        input.date,
        input.value_cents
    )
    .execute(pool)
    .await
    .map_err(|error| duplicate_date(error, input.date))?
    .last_insert_rowid();
    fetch(pool, id).await
}

pub async fn update(pool: &SqlitePool, id: i64, input: ValuationInput) -> AppResult<Valuation> {
    let current = fetch(pool, id).await?;
    check(pool, current.account_id, input).await?;
    sqlx::query!(
        "UPDATE valuation SET date = ?2, value_cents = ?3 WHERE id = ?1",
        id,
        input.date,
        input.value_cents
    )
    .execute(pool)
    .await
    .map_err(|error| duplicate_date(error, input.date))?;
    fetch(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let deleted = sqlx::query!("DELETE FROM valuation WHERE id = ?1", id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(AccountError::ValuationNotFound(id).into());
    }
    Ok(())
}

async fn fetch(pool: &SqlitePool, id: i64) -> AppResult<Valuation> {
    let valuation = sqlx::query_as!(
        Valuation,
        r#"SELECT id AS "id!", account_id, date AS "date: NaiveDate", value_cents
           FROM valuation
           WHERE id = ?1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(valuation.ok_or(AccountError::ValuationNotFound(id))?)
}

/// The account is valued (not tracked by operations), the value is not negative and the
/// date does not precede the account opening.
async fn check(pool: &SqlitePool, account_id: i64, input: ValuationInput) -> AppResult<()> {
    let account = get(pool, account_id).await?;
    if account.account_type.has_operations() {
        return Err(AccountError::NotValued.into());
    }
    if input.value_cents < 0 {
        return Err(AccountError::NegativeValuation.into());
    }
    if let Some(opening) = account.opening_date
        && input.date < opening
    {
        return Err(AccountError::ValuationBeforeOpening(opening).into());
    }
    Ok(())
}

fn duplicate_date(error: sqlx::Error, date: NaiveDate) -> AppError {
    match error.as_database_error() {
        Some(database) if database.is_unique_violation() => {
            AccountError::DuplicateValuationDate(date).into()
        }
        _ => error.into(),
    }
}

#[cfg(test)]
mod tests;
