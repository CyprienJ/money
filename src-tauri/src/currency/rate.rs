use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{CurrencyError, REFERENCE, check_supported};
use crate::error::AppResult;
use crate::investment::prices::{
    HistoryRange, PriceError, PriceFailure, PriceSource, PriceSourceKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRate {
    pub currency: String,
    pub date: NaiveDate,
    /// Euros for one unit of the currency, in billionths.
    pub rate_nano: i64,
    pub source: PriceSourceKind,
}

/// A currency other than euros used by an account or a security.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyUse {
    pub currency: String,
    /// Most recent rate; `None` when the currency has none, and its amounts count 0.
    pub latest: Option<ExchangeRate>,
}

/// Rates of a currency, the most recent first.
pub async fn list_rates(pool: &SqlitePool, currency: &str) -> AppResult<Vec<ExchangeRate>> {
    let rates = sqlx::query_as!(
        ExchangeRate,
        r#"SELECT currency, date AS "date: NaiveDate", rate_nano,
                  source AS "source: PriceSourceKind"
           FROM exchange_rate
           WHERE currency = ?1
           ORDER BY date DESC"#,
        currency
    )
    .fetch_all(pool)
    .await?;
    Ok(rates)
}

/// Records the rate of a day by hand, replacing any rate of that day.
pub async fn set_manual_rate(
    pool: &SqlitePool,
    currency: &str,
    date: NaiveDate,
    rate_nano: i64,
) -> AppResult<()> {
    let currency = check_supported(currency)?;
    if currency == REFERENCE {
        return Err(CurrencyError::ReferenceRate.into());
    }
    if rate_nano <= 0 {
        return Err(CurrencyError::InvalidRate.into());
    }
    sqlx::query!(
        "INSERT INTO exchange_rate (currency, date, rate_nano, source)
         VALUES (?1, ?2, ?3, 'MANUAL')
         ON CONFLICT (currency, date)
         DO UPDATE SET rate_nano = excluded.rate_nano, source = 'MANUAL'",
        currency,
        date,
        rate_nano
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_rate(pool: &SqlitePool, currency: &str, date: NaiveDate) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM exchange_rate WHERE currency = ?1 AND date = ?2",
        currency,
        date
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Currencies other than euros of the non-archived accounts and of the securities, by code.
pub async fn in_use(pool: &SqlitePool) -> AppResult<Vec<CurrencyUse>> {
    let currencies = sqlx::query_scalar!(
        r#"SELECT currency AS "currency!" FROM account WHERE archived = 0 AND currency <> ?1
           UNION
           SELECT currency FROM security WHERE currency <> ?1
           ORDER BY 1"#,
        REFERENCE
    )
    .fetch_all(pool)
    .await?;
    let mut uses = Vec::new();
    for currency in currencies {
        let latest = list_rates(pool, &currency).await?.into_iter().next();
        uses.push(CurrencyUse { currency, latest });
    }
    Ok(uses)
}

/// Fetches the recent rates of the currencies in use (a year for a currency without rates
/// yet), never replacing a rate entered by hand. Returns the number of currencies updated and
/// the failures, named "USD/EUR".
pub(crate) async fn update_rates(
    pool: &SqlitePool,
    source: &impl PriceSource,
    today: NaiveDate,
) -> AppResult<(u32, Vec<PriceFailure>)> {
    let mut updated = 0;
    let mut failures = Vec::new();
    for currency in in_use(pool).await? {
        let range = if currency.latest.is_some() {
            HistoryRange::Month
        } else {
            HistoryRange::Year
        };
        let symbol = format!("{}{REFERENCE}=X", currency.currency);
        let name = format!("{}/{REFERENCE}", currency.currency);
        let closes = match source.closes(&symbol, range).await {
            Ok(closes) if closes.currency == REFERENCE => closes,
            Ok(closes) => {
                let error = PriceError::UnexpectedResponse(format!(
                    "{symbol} is quoted in {}",
                    closes.currency
                ));
                failures.push(PriceFailure { name, error });
                continue;
            }
            Err(error) => {
                failures.push(PriceFailure { name, error });
                continue;
            }
        };
        let mut tx = pool.begin().await?;
        for (date, rate_nano) in closes.prices.into_iter().filter(|(date, _)| *date <= today) {
            sqlx::query!(
                "INSERT INTO exchange_rate (currency, date, rate_nano, source)
                 VALUES (?1, ?2, ?3, 'YAHOO')
                 ON CONFLICT (currency, date)
                 DO UPDATE SET rate_nano = excluded.rate_nano
                 WHERE exchange_rate.source = 'YAHOO'",
                currency.currency,
                date,
                rate_nano
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        updated += 1;
    }
    Ok((updated, failures))
}
