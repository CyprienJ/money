//! Import of the securities trades of a bank statement (Trade Republic) into the portfolio of
//! an investment account: preview of the trades, then import of the ones the user kept.
//!
//! A trade already in the account (same ISIN, date, kind, quantity, and amount plus fees) is a
//! duplicate, never imported. A security unknown by its ISIN is created with the name the
//! bank gives it, in the currency of the account. The amount is the total paid or received,
//! fees included: the statement does not detail them. The import makes a batch without
//! operations, which keeps the file: each trade opens its statement.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::batch::{self, BalanceCheckStatus, ImportError, SourceType};
use super::pdf::{self, PdfError, Trade};
use super::source_document;
use crate::account::{self, AccountType};
use crate::error::AppResult;
use crate::investment::{
    InvestmentError, SecurityInput, TransactionInput, TransactionKind, securities, transactions,
};

/// A statement file whose trades are imported.
#[derive(Debug, Clone, Copy)]
pub struct TradeFile<'a> {
    pub account_id: i64,
    pub file_name: &'a str,
    pub bytes: &'a [u8],
    /// Directory of the PDFium library.
    pub pdfium_dir: &'a Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TradeLine {
    pub trade: Trade,
    /// Security of the ISIN; `None`: created at import.
    pub security_id: Option<i64>,
    /// Already in the account: never imported.
    pub duplicate: bool,
    /// Dated before the account opening: never imported.
    pub before_opening: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TradePreview {
    pub file_name: String,
    /// To check at import time that the file did not change.
    pub file_hash: String,
    /// Bank recognized from the statement.
    pub bank: String,
    /// Trades in file order; the import refers to them by index.
    pub lines: Vec<TradeLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TradeImport {
    pub imported: u32,
    pub created_securities: u32,
}

/// What makes two trades the same: ISIN, date, kind, quantity and total amount.
type TradeKey = (String, NaiveDate, TransactionKind, i64, i64);

fn key(trade: &Trade) -> TradeKey {
    (
        trade.isin.clone(),
        trade.date,
        trade.kind,
        trade.quantity_micro,
        trade.amount_cents,
    )
}

/// Reads the trades of the statement and compares them with the portfolio of the account.
pub async fn preview(pool: &SqlitePool, file: TradeFile<'_>) -> AppResult<TradePreview> {
    let account = account::get(pool, file.account_id).await?;
    if account.account_type != AccountType::Investment {
        return Err(InvestmentError::NotAnInvestmentAccount.into());
    }
    let document = pdf::extract(file.pdfium_dir, file.bytes)?;
    let parser = pdf::detect(&document).ok_or(PdfError::UnknownFormat)?;
    let trades = parser
        .parse_trades(&document)?
        .ok_or_else(|| PdfError::NoTrades(parser.bank().to_owned()))?;

    let securities: HashMap<String, i64> = securities::list(pool)
        .await?
        .into_iter()
        .filter_map(|s| Some((s.isin?, s.id)))
        .collect();
    // Trades of the account, counted: two identical trades in the file need two in the account.
    let mut existing: HashMap<TradeKey, u32> = HashMap::new();
    let rows = sqlx::query!(
        r#"SELECT s.isin AS "isin!", t.date AS "date: NaiveDate", t.kind AS "kind: TransactionKind",
                  t.quantity_micro, t.amount_cents + t.fees_cents AS "total!: i64"
           FROM investment_transaction t
           JOIN security s ON s.id = t.security_id
           WHERE t.account_id = ?1 AND s.isin IS NOT NULL"#,
        file.account_id
    )
    .fetch_all(pool)
    .await?;
    for row in rows {
        let key = (row.isin, row.date, row.kind, row.quantity_micro, row.total);
        *existing.entry(key).or_default() += 1;
    }

    let lines = trades
        .into_iter()
        .map(|trade| {
            let duplicate = match existing.get_mut(&key(&trade)) {
                Some(count) if *count > 0 => {
                    *count -= 1;
                    true
                }
                _ => false,
            };
            TradeLine {
                security_id: securities.get(&trade.isin).copied(),
                before_opening: trade.date < account.opening_date,
                duplicate,
                trade,
            }
        })
        .collect();
    Ok(TradePreview {
        file_name: file.file_name.to_owned(),
        file_hash: batch::file_hash(file.bytes),
        bank: parser.bank().to_owned(),
        lines,
    })
}

/// Imports the selected trades, after a backup into `backup_dir`. The file is read again and
/// must not have changed since the preview; duplicates are refused. Missing securities are
/// created first, then every trade is recorded at once, buys before sells on a day. The file
/// is kept in `statements_dir`.
pub async fn import(
    pool: &SqlitePool,
    backup_dir: &Path,
    statements_dir: &Path,
    file: TradeFile<'_>,
    expected_hash: &str,
    indexes: &[u32],
) -> AppResult<TradeImport> {
    if batch::file_hash(file.bytes) != expected_hash {
        return Err(ImportError::FileChanged.into());
    }
    let preview = preview(pool, file).await?;
    let account = account::get(pool, file.account_id).await?;

    let mut seen = HashSet::new();
    let mut lines = Vec::new();
    for &index in indexes {
        if !seen.insert(index) {
            continue;
        }
        let line = preview
            .lines
            .get(index as usize)
            .ok_or(ImportError::LineOutOfRange(index))?;
        if line.duplicate {
            return Err(ImportError::DuplicateLine(index).into());
        }
        lines.push(line);
    }
    if lines.is_empty() {
        return Err(ImportError::NoLine.into());
    }

    batch::backup(pool, backup_dir).await?;

    let mut created: HashMap<&str, i64> = HashMap::new();
    for line in lines.iter().filter(|l| l.security_id.is_none()) {
        if created.contains_key(line.trade.isin.as_str()) {
            continue;
        }
        let security = securities::create(
            pool,
            SecurityInput {
                name: line.trade.name.clone(),
                isin: Some(line.trade.isin.clone()),
                symbol: None,
                currency: account.currency.clone(),
            },
        )
        .await?;
        created.insert(&line.trade.isin, security.id);
    }

    let mut inputs: Vec<(TransactionInput, Option<u32>)> = lines
        .iter()
        .map(|line| {
            let input = TransactionInput {
                security_id: line
                    .security_id
                    .or_else(|| created.get(line.trade.isin.as_str()).copied())
                    .expect("missing securities are created above"),
                date: line.trade.date,
                kind: line.trade.kind,
                quantity_micro: line.trade.quantity_micro,
                amount_cents: line.trade.amount_cents,
                fees_cents: 0,
                note: None,
            };
            (input, line.trade.source_page)
        })
        .collect();
    inputs.sort_by_key(|(input, _)| (input.date, input.kind != TransactionKind::Buy));

    let stored_file = source_document::store(
        statements_dir,
        &preview.file_hash,
        file.file_name,
        file.bytes,
    )?;
    let batch_id = sqlx::query!(
        "INSERT INTO import_batch
         (account_id, source_type, source_file_name, source_file_hash, imported_at,
          balance_check_status, stored_file)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        file.account_id,
        SourceType::Pdf,
        preview.file_name,
        preview.file_hash,
        crate::operation::timestamp(),
        BalanceCheckStatus::NotAvailable,
        stored_file
    )
    .execute(pool)
    .await?
    .last_insert_rowid();
    if let Err(error) = transactions::create_many(pool, file.account_id, batch_id, &inputs).await {
        // Nothing was recorded: the batch and the securities it created go, and its file too.
        sqlx::query!("DELETE FROM import_batch WHERE id = ?1", batch_id)
            .execute(pool)
            .await?;
        for &security_id in created.values() {
            securities::delete(pool, security_id).await?;
        }
        source_document::sweep(pool, statements_dir).await?;
        return Err(error);
    }
    Ok(TradeImport {
        imported: u32::try_from(inputs.len()).unwrap_or(u32::MAX),
        created_securities: u32::try_from(created.len()).unwrap_or(u32::MAX),
    })
}

#[cfg(test)]
mod tests;
