use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::{SqliteConnection, SqlitePool};

use super::InvestmentError;
use super::position::Holding;
use crate::account::{self, AccountType};
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionKind {
    Buy,
    Sell,
    Dividend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InvestmentTransaction {
    pub id: i64,
    pub account_id: i64,
    pub security_id: i64,
    pub security_name: String,
    pub date: NaiveDate,
    pub kind: TransactionKind,
    /// Millionths of a unit; 0 for a dividend.
    pub quantity_micro: i64,
    /// Quantity × unit price for a buy or a sell, amount received for a dividend.
    pub amount_cents: i64,
    pub fees_cents: i64,
    pub note: Option<String>,
    /// Gain of a sell: proceeds net of fees − cost of the units sold; `None` otherwise.
    pub realized_cents: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInput {
    pub security_id: i64,
    pub date: NaiveDate,
    pub kind: TransactionKind,
    pub quantity_micro: i64,
    pub amount_cents: i64,
    pub fees_cents: i64,
    pub note: Option<String>,
}

/// Transactions of an investment account, the most recent first.
pub async fn list(pool: &SqlitePool, account_id: i64) -> AppResult<Vec<InvestmentTransaction>> {
    investment_account(pool, account_id).await?;
    let transactions = sqlx::query_as!(
        InvestmentTransaction,
        r#"SELECT t.id AS "id!", t.account_id, t.security_id, s.name AS security_name,
                  t.date AS "date: NaiveDate", t.kind AS "kind: TransactionKind",
                  t.quantity_micro, t.amount_cents, t.fees_cents, t.note,
                  CAST(NULL AS INTEGER) AS "realized_cents?: i64"
           FROM investment_transaction t
           JOIN security s ON s.id = t.security_id
           WHERE t.account_id = ?1
           ORDER BY t.date DESC, t.id DESC"#,
        account_id
    )
    .fetch_all(pool)
    .await?;
    Ok(with_realized_gains(transactions))
}

/// Fills the gain of each sell, from the history of its security in chronological order.
fn with_realized_gains(mut transactions: Vec<InvestmentTransaction>) -> Vec<InvestmentTransaction> {
    let mut holdings: HashMap<i64, Holding> = HashMap::new();
    // Listed the most recent first.
    for transaction in transactions.iter_mut().rev() {
        let holding = holdings.entry(transaction.security_id).or_default();
        let after = holding
            .apply(
                transaction.kind,
                transaction.quantity_micro,
                transaction.amount_cents,
                transaction.fees_cents,
            )
            .unwrap_or_default();
        if transaction.kind == TransactionKind::Sell {
            transaction.realized_cents = Some(after.realized_cents - holding.realized_cents);
        }
        *holding = after;
    }
    transactions
}

async fn get(pool: &SqlitePool, id: i64) -> AppResult<InvestmentTransaction> {
    let transaction = sqlx::query_as!(
        InvestmentTransaction,
        r#"SELECT t.id AS "id!", t.account_id, t.security_id, s.name AS security_name,
                  t.date AS "date: NaiveDate", t.kind AS "kind: TransactionKind",
                  t.quantity_micro, t.amount_cents, t.fees_cents, t.note,
                  CAST(NULL AS INTEGER) AS "realized_cents?: i64"
           FROM investment_transaction t
           JOIN security s ON s.id = t.security_id
           WHERE t.id = ?1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(transaction.ok_or(InvestmentError::TransactionNotFound(id))?)
}

pub async fn create(
    pool: &SqlitePool,
    account_id: i64,
    input: TransactionInput,
) -> AppResult<InvestmentTransaction> {
    let note = check(pool, account_id, &input).await?;
    let mut tx = pool.begin().await?;
    let id = sqlx::query!(
        "INSERT INTO investment_transaction
         (account_id, security_id, date, kind, quantity_micro, amount_cents, fees_cents, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        account_id,
        input.security_id,
        input.date,
        input.kind,
        input.quantity_micro,
        input.amount_cents,
        input.fees_cents,
        note
    )
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    check_holdings(&mut tx, account_id, input.security_id).await?;
    tx.commit().await?;
    get(pool, id).await
}

/// Changes a transaction; its account stays the same.
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    input: TransactionInput,
) -> AppResult<InvestmentTransaction> {
    let current = get(pool, id).await?;
    let note = check(pool, current.account_id, &input).await?;
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "UPDATE investment_transaction
         SET security_id = ?2, date = ?3, kind = ?4, quantity_micro = ?5, amount_cents = ?6,
             fees_cents = ?7, note = ?8
         WHERE id = ?1",
        id,
        input.security_id,
        input.date,
        input.kind,
        input.quantity_micro,
        input.amount_cents,
        input.fees_cents,
        note
    )
    .execute(&mut *tx)
    .await?;
    check_holdings(&mut tx, current.account_id, input.security_id).await?;
    if current.security_id != input.security_id {
        check_holdings(&mut tx, current.account_id, current.security_id).await?;
    }
    tx.commit().await?;
    get(pool, id).await
}

/// Deletes a transaction, unless a later sell needs the units it bought.
pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let current = get(pool, id).await?;
    let mut tx = pool.begin().await?;
    sqlx::query!("DELETE FROM investment_transaction WHERE id = ?1", id)
        .execute(&mut *tx)
        .await?;
    check_holdings(&mut tx, current.account_id, current.security_id).await?;
    tx.commit().await?;
    Ok(())
}

async fn investment_account(pool: &SqlitePool, account_id: i64) -> AppResult<account::Account> {
    let account = account::get(pool, account_id).await?;
    if account.account_type != AccountType::Investment {
        return Err(InvestmentError::NotAnInvestmentAccount.into());
    }
    Ok(account)
}

/// Checks the input and returns the trimmed note.
async fn check(
    pool: &SqlitePool,
    account_id: i64,
    input: &TransactionInput,
) -> AppResult<Option<String>> {
    let account = investment_account(pool, account_id).await?;
    if input.date < account.opening_date {
        return Err(InvestmentError::DateBeforeOpening(account.opening_date).into());
    }
    match input.kind {
        TransactionKind::Buy | TransactionKind::Sell if input.quantity_micro <= 0 => {
            return Err(InvestmentError::MissingQuantity.into());
        }
        TransactionKind::Dividend if input.quantity_micro != 0 => {
            return Err(InvestmentError::QuantityOnDividend.into());
        }
        _ => {}
    }
    if input.amount_cents <= 0 || input.fees_cents < 0 {
        return Err(InvestmentError::ZeroAmount.into());
    }
    let exists = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM security WHERE id = ?1"#,
        input.security_id
    )
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        return Err(InvestmentError::SecurityNotFound(input.security_id).into());
    }
    Ok(input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned))
}

/// Refuses a history of a security in an account where a sell exceeds the units held.
async fn check_holdings(
    conn: &mut SqliteConnection,
    account_id: i64,
    security_id: i64,
) -> AppResult<()> {
    let rows = sqlx::query!(
        r#"SELECT t.date AS "date: NaiveDate", t.kind AS "kind: TransactionKind",
                  t.quantity_micro, t.amount_cents, t.fees_cents, s.name
           FROM investment_transaction t
           JOIN security s ON s.id = t.security_id
           WHERE t.account_id = ?1 AND t.security_id = ?2
           ORDER BY t.date, t.id"#,
        account_id,
        security_id
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut holding = Holding::default();
    for row in rows {
        holding = holding
            .apply(
                row.kind,
                row.quantity_micro,
                row.amount_cents,
                row.fees_cents,
            )
            .ok_or(InvestmentError::Oversold {
                date: row.date,
                security: row.name,
            })?;
    }
    Ok(())
}
