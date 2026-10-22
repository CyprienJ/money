//! Security prices: entered by hand, or fetched from Yahoo Finance once the user enabled it
//! (the only network access of the application). Fetched prices never overwrite a price
//! entered by hand. Only prices in euros are kept until multi-currency support.

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::InvestmentError;
use crate::error::{AppResult, ErrorKind};
use crate::setting;

const UPDATES_ENABLED: &str = "price_updates_enabled";
const UPDATED_ON: &str = "prices_updated_on";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PriceSourceKind {
    Manual,
    Yahoo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPrice {
    pub security_id: i64,
    pub date: NaiveDate,
    /// Price of one unit.
    pub price_cents: i64,
    pub source: PriceSourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PriceSettings {
    /// The user allowed fetching prices from Yahoo Finance.
    pub updates_enabled: bool,
    /// Day of the last update.
    pub updated_on: Option<NaiveDate>,
}

/// A listing of a security found by its ISIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SymbolCandidate {
    pub symbol: String,
    pub name: String,
    /// Exchange, e.g. "Paris" or "Amsterdam".
    pub exchange: String,
}

/// Result of an update of the prices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PriceUpdate {
    /// Securities whose prices were fetched.
    pub updated: u32,
    pub failures: Vec<PriceFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PriceFailure {
    pub security_name: String,
    pub error: PriceError,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PriceError {
    #[error("fetching prices is disabled in the settings")]
    Disabled,

    #[error("network error: {0}")]
    Network(String),

    #[error("unexpected response of the price source: {0}")]
    UnexpectedResponse(String),

    #[error("the symbol {0} is unknown to the price source")]
    UnknownSymbol(String),

    #[error("the prices of {symbol} are in {currency}, only euros are supported")]
    UnsupportedCurrency { symbol: String, currency: String },

    #[error("the price must be positive")]
    InvalidPrice,
}

impl PriceError {
    pub fn kind(&self) -> ErrorKind {
        ErrorKind::Validation
    }
}

/// Period of the closes to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryRange {
    Month,
    /// For a security without prices yet.
    Year,
}

/// Daily closes of a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closes {
    pub currency: String,
    pub prices: Vec<(NaiveDate, i64)>,
}

/// A source of prices, Yahoo Finance or a fake one in tests.
pub trait PriceSource {
    fn closes(
        &self,
        symbol: &str,
        range: HistoryRange,
    ) -> impl Future<Output = Result<Closes, PriceError>> + Send;
    fn search(
        &self,
        isin: &str,
    ) -> impl Future<Output = Result<Vec<SymbolCandidate>, PriceError>> + Send;
}

pub async fn settings(pool: &SqlitePool) -> AppResult<PriceSettings> {
    Ok(PriceSettings {
        updates_enabled: setting::get(pool, UPDATES_ENABLED).await?.as_deref() == Some("true"),
        updated_on: setting::get(pool, UPDATED_ON)
            .await?
            .and_then(|date| date.parse().ok()),
    })
}

pub async fn set_updates_enabled(pool: &SqlitePool, enabled: bool) -> AppResult<PriceSettings> {
    setting::set(
        pool,
        UPDATES_ENABLED,
        if enabled { "true" } else { "false" },
    )
    .await?;
    settings(pool).await
}

async fn ensure_enabled(pool: &SqlitePool) -> AppResult<()> {
    if !settings(pool).await?.updates_enabled {
        return Err(PriceError::Disabled.into());
    }
    Ok(())
}

/// Prices of a security, the most recent first.
pub async fn list(pool: &SqlitePool, security_id: i64) -> AppResult<Vec<SecurityPrice>> {
    let prices = sqlx::query_as!(
        SecurityPrice,
        r#"SELECT security_id, date AS "date: NaiveDate", price_cents,
                  source AS "source: PriceSourceKind"
           FROM security_price
           WHERE security_id = ?1
           ORDER BY date DESC"#,
        security_id
    )
    .fetch_all(pool)
    .await?;
    Ok(prices)
}

/// Records the price of a day by hand, replacing any price of that day.
pub async fn set_manual(
    pool: &SqlitePool,
    security_id: i64,
    date: NaiveDate,
    price_cents: i64,
) -> AppResult<()> {
    if price_cents <= 0 {
        return Err(PriceError::InvalidPrice.into());
    }
    let exists = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM security WHERE id = ?1"#,
        security_id
    )
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        return Err(InvestmentError::SecurityNotFound(security_id).into());
    }
    sqlx::query!(
        "INSERT INTO security_price (security_id, date, price_cents, source)
         VALUES (?1, ?2, ?3, 'MANUAL')
         ON CONFLICT (security_id, date)
         DO UPDATE SET price_cents = excluded.price_cents, source = 'MANUAL'",
        security_id,
        date,
        price_cents
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, security_id: i64, date: NaiveDate) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM security_price WHERE security_id = ?1 AND date = ?2",
        security_id,
        date
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Listings of a security found by its ISIN; refused while fetching is disabled.
pub async fn search_symbols(
    pool: &SqlitePool,
    source: &impl PriceSource,
    isin: &str,
) -> AppResult<Vec<SymbolCandidate>> {
    ensure_enabled(pool).await?;
    Ok(source.search(isin.trim()).await?)
}

/// Fetches the recent closes of every security with a symbol (a year for a security without
/// prices yet), and records the day of the update. Refused while fetching is disabled.
pub async fn update(
    pool: &SqlitePool,
    source: &impl PriceSource,
    today: NaiveDate,
) -> AppResult<PriceUpdate> {
    ensure_enabled(pool).await?;
    let securities = sqlx::query!(
        r#"SELECT s.id AS "id!", s.name, s.symbol AS "symbol!",
                  EXISTS (SELECT 1 FROM security_price p WHERE p.security_id = s.id)
                      AS "has_prices!: bool"
           FROM security s
           WHERE s.symbol IS NOT NULL
           ORDER BY s.name"#
    )
    .fetch_all(pool)
    .await?;
    let mut result = PriceUpdate {
        updated: 0,
        failures: Vec::new(),
    };
    for security in securities {
        let range = if security.has_prices {
            HistoryRange::Month
        } else {
            HistoryRange::Year
        };
        let closes = match source.closes(&security.symbol, range).await {
            Ok(closes) if closes.currency == "EUR" => closes,
            Ok(closes) => {
                result.failures.push(PriceFailure {
                    security_name: security.name,
                    error: PriceError::UnsupportedCurrency {
                        symbol: security.symbol,
                        currency: closes.currency,
                    },
                });
                continue;
            }
            Err(error) => {
                result.failures.push(PriceFailure {
                    security_name: security.name,
                    error,
                });
                continue;
            }
        };
        let mut tx = pool.begin().await?;
        for (date, price_cents) in closes.prices.into_iter().filter(|(date, _)| *date <= today) {
            sqlx::query!(
                "INSERT INTO security_price (security_id, date, price_cents, source)
                 VALUES (?1, ?2, ?3, 'YAHOO')
                 ON CONFLICT (security_id, date)
                 DO UPDATE SET price_cents = excluded.price_cents
                 WHERE security_price.source = 'YAHOO'",
                security.id,
                date,
                price_cents
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        result.updated += 1;
    }
    setting::set(pool, UPDATED_ON, &today.to_string()).await?;
    Ok(result)
}

/// Whether the daily update is due: enabled and not done today.
pub async fn update_due(pool: &SqlitePool, today: NaiveDate) -> AppResult<bool> {
    let settings = settings(pool).await?;
    Ok(settings.updates_enabled && settings.updated_on != Some(today))
}

#[cfg(test)]
mod tests;
