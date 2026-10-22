//! Yahoo Finance, the source of the fetched prices: its `chart` endpoint for daily closes and
//! its `search` endpoint to find the symbols of an ISIN. Unofficial: the responses are checked
//! and any surprise becomes a `PriceError`. Only symbols and ISINs are sent.

use std::time::Duration;

use chrono::{DateTime, NaiveDate};
use serde::Deserialize;
use serde_json::Number;

use super::prices::{Closes, HistoryRange, PriceError, PriceSource, SymbolCandidate};
use crate::currency::RATE_SCALE;

const CHART_URL: &str = "https://query1.finance.yahoo.com/v8/finance/chart/";
const SEARCH_URL: &str = "https://query1.finance.yahoo.com/v1/finance/search";

pub struct Yahoo {
    client: reqwest::Client,
}

impl Yahoo {
    pub fn new() -> Result<Yahoo, PriceError> {
        let client = reqwest::Client::builder()
            // The default user agent of HTTP libraries is refused.
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) Finances-perso")
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|error| PriceError::Network(error.to_string()))?;
        Ok(Yahoo { client })
    }

    async fn get(&self, url: &str, query: &[(&str, &str)]) -> Result<String, PriceError> {
        let response = self
            .client
            .get(url)
            .query(query)
            .send()
            .await
            .map_err(|error| PriceError::Network(error.to_string()))?;
        // A 404 carries a JSON error, read by the caller.
        response
            .text()
            .await
            .map_err(|error| PriceError::Network(error.to_string()))
    }
}

impl PriceSource for Yahoo {
    async fn closes(&self, symbol: &str, range: HistoryRange) -> Result<Closes, PriceError> {
        let range = match range {
            HistoryRange::Month => "1mo",
            HistoryRange::Year => "1y",
        };
        let url = format!("{CHART_URL}{symbol}");
        let body = self
            .get(&url, &[("range", range), ("interval", "1d")])
            .await?;
        parse_chart(symbol, &body)
    }

    async fn search(&self, isin: &str) -> Result<Vec<SymbolCandidate>, PriceError> {
        let body = self
            .get(
                SEARCH_URL,
                &[("q", isin), ("quotesCount", "10"), ("newsCount", "0")],
            )
            .await?;
        parse_search(&body)
    }
}

#[derive(Deserialize)]
struct ChartResponse {
    chart: Chart,
}

#[derive(Deserialize)]
struct Chart {
    result: Option<Vec<ChartResult>>,
    error: Option<ChartError>,
}

#[derive(Deserialize)]
struct ChartError {
    description: Option<String>,
}

#[derive(Deserialize)]
struct ChartResult {
    meta: Meta,
    #[serde(default)]
    timestamp: Vec<i64>,
    indicators: Indicators,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    currency: Option<String>,
    #[serde(default)]
    gmtoffset: i64,
}

#[derive(Deserialize)]
struct Indicators {
    quote: Vec<Quote>,
}

#[derive(Deserialize)]
struct Quote {
    #[serde(default)]
    close: Vec<Option<Number>>,
}

/// Daily closes of a `chart` response, dated in the time zone of the exchange.
pub(super) fn parse_chart(symbol: &str, body: &str) -> Result<Closes, PriceError> {
    let response: ChartResponse = serde_json::from_str(body)
        .map_err(|error| PriceError::UnexpectedResponse(error.to_string()))?;
    let Some(result) = response.chart.result.and_then(|r| r.into_iter().next()) else {
        let reason = response.chart.error.and_then(|e| e.description);
        return Err(match reason {
            Some(_) => PriceError::UnknownSymbol(symbol.to_owned()),
            None => PriceError::UnexpectedResponse("empty chart".to_owned()),
        });
    };
    let currency = result.meta.currency.unwrap_or_default();
    let closes = result
        .indicators
        .quote
        .into_iter()
        .next()
        .map(|q| q.close)
        .unwrap_or_default();
    let prices = result
        .timestamp
        .iter()
        .zip(closes)
        .filter_map(|(timestamp, close)| {
            let date = DateTime::from_timestamp(timestamp + result.meta.gmtoffset, 0)?.date_naive();
            Some((date, nano(&close?.to_string())?))
        })
        .collect::<Vec<(NaiveDate, i64)>>();
    Ok(Closes { currency, prices })
}

/// Billionths of a decimal number as written in JSON (a price or a rate), rounded half up,
/// without going through floating point; `None` for a negative, zero or exponent notation.
pub(super) fn nano(text: &str) -> Option<i64> {
    let (units, decimals) = text.split_once('.').unwrap_or((text, ""));
    if units.is_empty()
        || !units.bytes().all(|b| b.is_ascii_digit())
        || !decimals.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let digit = |index: usize| -> i64 {
        decimals
            .as_bytes()
            .get(index)
            .map_or(0, |b| i64::from(b - b'0'))
    };
    let units: i64 = units.parse().ok()?;
    let fraction = (0..9).fold(0, |value, index| value * 10 + digit(index));
    let nano = units
        .checked_mul(RATE_SCALE)?
        .checked_add(fraction + i64::from(digit(9) >= 5))?;
    (nano > 0).then_some(nano)
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    quotes: Vec<SearchQuote>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchQuote {
    symbol: Option<String>,
    shortname: Option<String>,
    longname: Option<String>,
    exch_disp: Option<String>,
    exchange: Option<String>,
}

/// Listings of a `search` response.
pub(super) fn parse_search(body: &str) -> Result<Vec<SymbolCandidate>, PriceError> {
    let response: SearchResponse = serde_json::from_str(body)
        .map_err(|error| PriceError::UnexpectedResponse(error.to_string()))?;
    Ok(response
        .quotes
        .into_iter()
        .filter_map(|quote| {
            Some(SymbolCandidate {
                symbol: quote.symbol?,
                name: quote.longname.or(quote.shortname).unwrap_or_default(),
                exchange: quote.exch_disp.or(quote.exchange).unwrap_or_default(),
            })
        })
        .collect())
}
