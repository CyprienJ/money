//! Internal transfer pairing: a debit on one account is linked to the matching credit on
//! another (opposite amount, close dates). Linked operations get the internal transfer
//! category, which excludes them from income and expense statistics.
//!
//! A pair is certain when each operation is the only candidate of the other: certain pairs are
//! linked automatically, the others are proposed to the user, who links or dismisses them.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::{SqliteConnection, SqlitePool};

use super::OperationError;
use super::repository::timestamp;
use crate::error::{AppError, AppResult, ErrorKind};

#[cfg(test)]
mod tests;

/// Largest gap, in days, between the debit and the credit of a detected transfer.
pub const DETECTION_WINDOW_DAYS: i64 = 3;

/// Largest gap, in days, between two operations the user links by hand.
pub const MANUAL_WINDOW_DAYS: i64 = 15;

/// One side of a transfer, as shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferSide {
    pub operation_id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub date: NaiveDate,
    pub amount_cents: i64,
    /// Payee name, or the normalized label when the operation has no payee.
    pub payee_name: String,
}

/// A detected transfer the user has to confirm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferProposal {
    pub debit: TransferSide,
    pub credit: TransferSide,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TransferError {
    #[error("both operations of a transfer must be on different accounts")]
    SameAccount,

    #[error("the amounts of a transfer must be opposite")]
    AmountsNotOpposite,

    #[error("both accounts of a transfer must have the same currency")]
    DifferentCurrencies,

    #[error("operation {0} is already linked to another transfer")]
    AlreadyLinked(i64),
}

impl TransferError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::Validation
    }
}

/// A candidate pair: a debit and a credit that may be the two sides of a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Candidate {
    debit_id: i64,
    credit_id: i64,
}

/// Unlinked debits and credits of different accounts of the same currency with opposite
/// amounts, at most `DETECTION_WINDOW_DAYS` apart, both uncategorized or internal transfers,
/// and not dismissed.
async fn candidates(pool: &SqlitePool) -> AppResult<Vec<Candidate>> {
    let rows = sqlx::query!(
        r#"SELECT d.id AS "debit_id!", c.id AS "credit_id!"
           FROM operation d
           JOIN operation c
             ON c.amount_cents = -d.amount_cents
            AND c.account_id <> d.account_id
            AND c.currency = d.currency
            AND ABS(julianday(c.date) - julianday(d.date)) <= ?1
           JOIN operation_split ds ON ds.operation_id = d.id
           JOIN operation_split cs ON cs.operation_id = c.id
           WHERE d.amount_cents < 0
             AND d.transfer_pair_id IS NULL AND c.transfer_pair_id IS NULL
             AND (ds.category_id IS NULL OR ds.category_id IN
                     (SELECT id FROM category WHERE is_internal_transfer = 1))
             AND (cs.category_id IS NULL OR cs.category_id IN
                     (SELECT id FROM category WHERE is_internal_transfer = 1))
             AND NOT EXISTS (SELECT 1 FROM transfer_dismissal t
                             WHERE t.debit_id = d.id AND t.credit_id = c.id)
           ORDER BY d.date DESC, d.id DESC, c.date, c.id"#,
        DETECTION_WINDOW_DAYS
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| Candidate {
            debit_id: row.debit_id,
            credit_id: row.credit_id,
        })
        .collect())
}

/// Splits the candidates into certain pairs (each side has no other candidate) and the others.
fn split_certain(candidates: &[Candidate]) -> (Vec<Candidate>, Vec<Candidate>) {
    let mut debits: HashMap<i64, usize> = HashMap::new();
    let mut credits: HashMap<i64, usize> = HashMap::new();
    for candidate in candidates {
        *debits.entry(candidate.debit_id).or_default() += 1;
        *credits.entry(candidate.credit_id).or_default() += 1;
    }
    candidates
        .iter()
        .partition(|c| debits[&c.debit_id] == 1 && credits[&c.credit_id] == 1)
}

/// Links the certain pairs; with `batch_id`, only those with a side in that import. Returns
/// the count of transfers linked.
pub async fn link_certain(pool: &SqlitePool, batch_id: Option<i64>) -> AppResult<u32> {
    let (certain, _) = split_certain(&candidates(pool).await?);
    let mut tx = pool.begin().await?;
    let mut linked = 0;
    for pair in certain {
        if let Some(batch_id) = batch_id {
            let in_batch = sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!: i64" FROM operation
                   WHERE id IN (?1, ?2) AND import_batch_id = ?3"#,
                pair.debit_id,
                pair.credit_id,
                batch_id
            )
            .fetch_one(&mut *tx)
            .await?;
            if in_batch == 0 {
                continue;
            }
        }
        write_link(&mut tx, pair.debit_id, pair.credit_id).await?;
        linked += 1;
    }
    tx.commit().await?;
    Ok(linked)
}

/// Detected transfers that are not certain, the most recent debit first.
pub async fn proposals(pool: &SqlitePool) -> AppResult<Vec<TransferProposal>> {
    let (_, uncertain) = split_certain(&candidates(pool).await?);
    let mut conn = pool.acquire().await?;
    let mut proposals = Vec::with_capacity(uncertain.len());
    for pair in uncertain {
        proposals.push(TransferProposal {
            debit: side(&mut conn, pair.debit_id).await?,
            credit: side(&mut conn, pair.credit_id).await?,
        });
    }
    Ok(proposals)
}

/// Links two operations as the two sides of a transfer, and gives both the internal transfer
/// category. They must be on different accounts of the same currency, with opposite amounts,
/// and not linked yet.
pub async fn link(pool: &SqlitePool, id: i64, other_id: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let first = pairing_state(&mut tx, id).await?;
    let second = pairing_state(&mut tx, other_id).await?;
    if first.account_id == second.account_id {
        return Err(TransferError::SameAccount.into());
    }
    if first.currency != second.currency {
        return Err(TransferError::DifferentCurrencies.into());
    }
    if first.amount_cents != -second.amount_cents {
        return Err(TransferError::AmountsNotOpposite.into());
    }
    for state in [&first, &second] {
        if state.transfer_pair_id.is_some() {
            return Err(TransferError::AlreadyLinked(state.id).into());
        }
    }
    write_link(&mut tx, id, other_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Unlinks an operation from its transfer, both sides; their categories are kept.
pub async fn unlink(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    pairing_state(&mut tx, id).await?;
    unlink_conn(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}

/// Dismisses a proposed pair, which is not proposed again.
pub async fn dismiss(pool: &SqlitePool, debit_id: i64, credit_id: i64) -> AppResult<()> {
    sqlx::query!(
        "INSERT OR IGNORE INTO transfer_dismissal (debit_id, credit_id) VALUES (?1, ?2)",
        debit_id,
        credit_id
    )
    .execute(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(ref db) if db.is_foreign_key_violation() => {
            AppError::from(OperationError::NotFound(debit_id.min(credit_id)))
        }
        error => error.into(),
    })?;
    Ok(())
}

/// The other side of the transfer of an operation, if it is linked.
pub async fn partner(pool: &SqlitePool, id: i64) -> AppResult<Option<TransferSide>> {
    let mut conn = pool.acquire().await?;
    match pairing_state(&mut conn, id).await?.transfer_pair_id {
        Some(pair_id) => Ok(Some(side(&mut conn, pair_id).await?)),
        None => Ok(None),
    }
}

/// Operations the user may link to an operation by hand: unlinked, on another account, with the
/// opposite amount, at most `MANUAL_WINDOW_DAYS` apart, whatever their category; the closest
/// dates first.
pub async fn link_candidates(pool: &SqlitePool, id: i64) -> AppResult<Vec<TransferSide>> {
    let mut conn = pool.acquire().await?;
    pairing_state(&mut conn, id).await?;
    let ids = sqlx::query_scalar!(
        r#"SELECT c.id AS "id!"
           FROM operation o
           JOIN operation c
             ON c.amount_cents = -o.amount_cents
            AND c.account_id <> o.account_id
            AND c.currency = o.currency
            AND ABS(julianday(c.date) - julianday(o.date)) <= ?2
           WHERE o.id = ?1 AND c.transfer_pair_id IS NULL
           ORDER BY ABS(julianday(c.date) - julianday(o.date)), c.date, c.id"#,
        id,
        MANUAL_WINDOW_DAYS
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut sides = Vec::with_capacity(ids.len());
    for candidate_id in ids {
        sides.push(side(&mut conn, candidate_id).await?);
    }
    Ok(sides)
}

/// Unlinks an operation and its partner, if linked. Used when a linked operation leaves the
/// internal transfer category.
pub(crate) async fn unlink_conn(conn: &mut SqliteConnection, id: i64) -> AppResult<()> {
    let now = timestamp();
    sqlx::query!(
        "UPDATE operation SET transfer_pair_id = NULL, updated_at = ?2
         WHERE id = ?1 OR transfer_pair_id = ?1",
        id,
        now
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Links both operations to each other, in the internal transfer category.
async fn write_link(conn: &mut SqliteConnection, id: i64, other_id: i64) -> AppResult<()> {
    let category_id = sqlx::query_scalar!(
        r#"SELECT id AS "id!" FROM category WHERE is_internal_transfer = 1 LIMIT 1"#
    )
    .fetch_one(&mut *conn)
    .await?;
    let now = timestamp();
    for (operation_id, pair_id) in [(id, other_id), (other_id, id)] {
        sqlx::query!(
            "UPDATE operation SET transfer_pair_id = ?2, is_internal_transfer = 1, updated_at = ?3
             WHERE id = ?1",
            operation_id,
            pair_id,
            now
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!(
            "UPDATE operation_split SET category_id = ?2 WHERE operation_id = ?1",
            operation_id,
            category_id
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

struct PairingState {
    id: i64,
    account_id: i64,
    amount_cents: i64,
    currency: String,
    transfer_pair_id: Option<i64>,
}

async fn pairing_state(conn: &mut SqliteConnection, id: i64) -> AppResult<PairingState> {
    let state = sqlx::query_as!(
        PairingState,
        "SELECT id, account_id, amount_cents, currency, transfer_pair_id FROM operation
         WHERE id = ?1",
        id
    )
    .fetch_optional(conn)
    .await?;
    Ok(state.ok_or(OperationError::NotFound(id))?)
}

async fn side(conn: &mut SqliteConnection, id: i64) -> AppResult<TransferSide> {
    let side = sqlx::query_as!(
        TransferSide,
        r#"SELECT o.id AS operation_id, o.account_id, a.name AS account_name,
                  o.date AS "date: NaiveDate", o.amount_cents,
                  COALESCE(p.name, o.normalized_label) AS "payee_name!: String"
           FROM operation o
           JOIN account a ON a.id = o.account_id
           LEFT JOIN payee p ON p.id = o.payee_id
           WHERE o.id = ?1"#,
        id
    )
    .fetch_optional(conn)
    .await?;
    Ok(side.ok_or(OperationError::NotFound(id))?)
}
