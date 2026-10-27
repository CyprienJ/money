use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::InvestmentError;
use crate::currency::{self, CurrencyError};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Security {
    pub id: i64,
    pub name: String,
    pub isin: Option<String>,
    /// Yahoo Finance symbol of the listing whose prices are fetched, e.g. `CW8.PA`.
    pub symbol: Option<String>,
    /// Currency of the prices; fixed once the security has prices.
    pub currency: String,
    /// Transactions of the security, in every account.
    pub transaction_count: i64,
    pub has_prices: bool,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SecurityInput {
    pub name: String,
    pub isin: Option<String>,
    pub symbol: Option<String>,
    pub currency: String,
}

/// Every security, by name.
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Security>> {
    let securities = sqlx::query_as!(
        Security,
        r#"SELECT s.id AS "id!", s.name, s.isin, s.symbol, s.currency,
                  (SELECT COUNT(*) FROM investment_transaction t WHERE t.security_id = s.id)
                      AS "transaction_count!: i64",
                  EXISTS (SELECT 1 FROM security_price p WHERE p.security_id = s.id)
                      AS "has_prices!: bool"
           FROM security s
           ORDER BY s.name COLLATE NOCASE"#
    )
    .fetch_all(pool)
    .await?;
    Ok(securities)
}

async fn get(pool: &SqlitePool, id: i64) -> AppResult<Security> {
    list(pool)
        .await?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| InvestmentError::SecurityNotFound(id).into())
}

pub async fn create(pool: &SqlitePool, input: SecurityInput) -> AppResult<Security> {
    let (name, isin, symbol) = clean(&input)?;
    let currency = currency::check_supported(&input.currency)?;
    let id = sqlx::query!(
        "INSERT INTO security (name, isin, symbol, currency) VALUES (?1, ?2, ?3, ?4)",
        name,
        isin,
        symbol,
        currency
    )
    .execute(pool)
    .await
    .map_err(|error| isin_taken(error, isin.as_deref()))?
    .last_insert_rowid();
    get(pool, id).await
}

pub async fn update(pool: &SqlitePool, id: i64, input: SecurityInput) -> AppResult<Security> {
    let (name, isin, symbol) = clean(&input)?;
    let currency = currency::check_supported(&input.currency)?;
    let current = get(pool, id).await?;
    if current.has_prices && currency != current.currency {
        return Err(CurrencyError::SecurityCurrencyLocked.into());
    }
    sqlx::query!(
        "UPDATE security SET name = ?2, isin = ?3, symbol = ?4, currency = ?5 WHERE id = ?1",
        id,
        name,
        isin,
        symbol,
        currency
    )
    .execute(pool)
    .await
    .map_err(|error| isin_taken(error, isin.as_deref()))?;
    get(pool, id).await
}

/// Deletes a security without transactions.
pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    if get(pool, id).await?.transaction_count > 0 {
        return Err(InvestmentError::SecurityInUse.into());
    }
    sqlx::query!("DELETE FROM security WHERE id = ?1", id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Trimmed name, ISIN and symbol in uppercase without spaces (`None` when empty).
fn clean(
    input: &SecurityInput,
) -> Result<(String, Option<String>, Option<String>), InvestmentError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(InvestmentError::EmptySecurityName);
    }
    let isin = input
        .isin
        .as_deref()
        .map(|isin| isin.split_whitespace().collect::<String>().to_uppercase())
        .filter(|isin| !isin.is_empty());
    if let Some(isin) = &isin
        && !is_valid_isin(isin)
    {
        return Err(InvestmentError::InvalidIsin(isin.clone()));
    }
    let symbol = input
        .symbol
        .as_deref()
        .map(|symbol| symbol.trim().to_uppercase())
        .filter(|symbol| !symbol.is_empty());
    if let Some(symbol) = &symbol
        && !symbol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '^' | '='))
    {
        return Err(InvestmentError::InvalidSymbol(symbol.clone()));
    }
    Ok((name.to_owned(), isin, symbol))
}

/// 2 letters, 9 letters or digits and a check digit (Luhn on the digits of the code, each
/// letter written as its number from A = 10 to Z = 35).
pub(super) fn is_valid_isin(isin: &str) -> bool {
    let bytes = isin.as_bytes();
    if bytes.len() != 12
        || !bytes[..2].iter().all(u8::is_ascii_uppercase)
        || !bytes[2..11]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        || !bytes[11].is_ascii_digit()
    {
        return false;
    }
    let digits: Vec<u32> = bytes
        .iter()
        .flat_map(|b| {
            let value = (*b as char).to_digit(36).expect("checked above");
            if value >= 10 {
                vec![value / 10, value % 10]
            } else {
                vec![value]
            }
        })
        .collect();
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(position, &digit)| {
            if position % 2 == 1 {
                let doubled = digit * 2;
                doubled / 10 + doubled % 10
            } else {
                digit
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn isin_taken(error: sqlx::Error, isin: Option<&str>) -> AppError {
    match (&error, isin) {
        (sqlx::Error::Database(db), Some(isin)) if db.is_unique_violation() => {
            InvestmentError::IsinTaken(isin.to_owned()).into()
        }
        _ => error.into(),
    }
}
