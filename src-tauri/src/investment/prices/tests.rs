use std::sync::Mutex;

use super::*;
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::investment::yahoo::{nano, parse_chart, parse_search};
use crate::investment::{SecurityInput, securities};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

#[test]
fn reads_prices_in_billionths_without_floating_point() {
    assert_eq!(nano("123.4"), Some(123_400_000_000));
    assert_eq!(nano("99.37999725341797"), Some(99_379_997_253));
    assert_eq!(nano("100"), Some(100_000_000_000));
    assert_eq!(nano("0.8924"), Some(892_400_000));
    assert_eq!(nano("0.0000000004"), None);
    assert_eq!(nano("1e-5"), None);
    assert_eq!(nano("-1.5"), None);
}

#[test]
fn parses_the_daily_closes_of_a_chart() {
    // 2025-10-19, 20 and 21 at 09:00 in Paris (UTC+2), the second close missing.
    let body = r#"{"chart":{"result":[{"meta":{"currency":"EUR","symbol":"CW8.PA","gmtoffset":7200},
        "timestamp":[1760857200,1760943600,1761030000],
        "indicators":{"quote":[{"close":[512.25,null,515.3999938964844]}]}}],"error":null}}"#;
    let closes = parse_chart("CW8.PA", body).unwrap();
    assert_eq!(closes.currency, "EUR");
    assert_eq!(
        closes.prices,
        [
            (date("2025-10-19"), 512_250_000_000),
            (date("2025-10-21"), 515_399_993_896)
        ]
    );

    let unknown = r#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found, symbol may be delisted"}}}"#;
    assert_eq!(
        parse_chart("XXX.PA", unknown),
        Err(PriceError::UnknownSymbol("XXX.PA".to_owned()))
    );
    assert!(matches!(
        parse_chart("CW8.PA", "<html>"),
        Err(PriceError::UnexpectedResponse(_))
    ));
}

#[test]
fn parses_the_listings_of_a_search() {
    let body = r#"{"quotes":[
        {"exchange":"PAR","shortname":"AMUNDI MSCI WORLD","longname":"Amundi MSCI World UCITS ETF","symbol":"CW8.PA","exchDisp":"Paris"},
        {"exchange":"MIL","shortname":"AMUNDI MSCI WORLD","symbol":"CW8.MI"},
        {"exchange":"PAR"}],"news":[]}"#;
    assert_eq!(
        parse_search(body).unwrap(),
        [
            SymbolCandidate {
                symbol: "CW8.PA".to_owned(),
                name: "Amundi MSCI World UCITS ETF".to_owned(),
                exchange: "Paris".to_owned(),
            },
            SymbolCandidate {
                symbol: "CW8.MI".to_owned(),
                name: "AMUNDI MSCI WORLD".to_owned(),
                exchange: "MIL".to_owned(),
            },
        ]
    );
}

/// Answers from a table of symbols, and records the ranges asked.
struct FakeSource {
    asked: Mutex<Vec<(String, HistoryRange)>>,
}

impl PriceSource for FakeSource {
    async fn closes(&self, symbol: &str, range: HistoryRange) -> Result<Closes, PriceError> {
        self.asked.lock().unwrap().push((symbol.to_owned(), range));
        match symbol {
            "CW8.PA" => Ok(Closes {
                currency: "EUR".to_owned(),
                prices: vec![
                    (date("2026-10-19"), 512_250_000_000),
                    (date("2026-10-20"), 515_400_000_000),
                    // After today: ignored.
                    (date("2026-10-21"), 1_000_000_000),
                ],
            }),
            // In pence: 90.155 pounds.
            "VUSA.L" => Ok(Closes {
                currency: "GBp".to_owned(),
                prices: vec![(date("2026-10-20"), 9_015_500_000_000)],
            }),
            "AAPL" => Ok(Closes {
                currency: "USD".to_owned(),
                prices: vec![(date("2026-10-20"), 250_000_000_000)],
            }),
            "7203.T" => Ok(Closes {
                currency: "JPY".to_owned(),
                prices: vec![(date("2026-10-20"), 3_000_000_000_000)],
            }),
            "GBPEUR=X" => Ok(Closes {
                currency: "EUR".to_owned(),
                prices: vec![(date("2026-10-20"), 1_150_000_000)],
            }),
            other => Err(PriceError::UnknownSymbol(other.to_owned())),
        }
    }

    async fn search(&self, _: &str) -> Result<Vec<SymbolCandidate>, PriceError> {
        Ok(Vec::new())
    }
}

async fn security(pool: &SqlitePool, name: &str, symbol: Option<&str>) -> i64 {
    securities::create(
        pool,
        SecurityInput {
            name: name.to_owned(),
            isin: None,
            symbol: symbol.map(str::to_owned),
            currency: "EUR".to_owned(),
        },
    )
    .await
    .unwrap()
    .id
}

#[test]
fn updates_the_prices_once_allowed_and_keeps_the_manual_ones() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let source = FakeSource {
            asked: Mutex::new(Vec::new()),
        };
        let today = date("2026-10-20");
        let world = security(&pool, "MSCI World", Some("CW8.PA")).await;
        let sp500 = security(&pool, "S&P 500", Some("VUSA.L")).await;
        security(&pool, "Délisté", Some("OLD.PA")).await;
        security(&pool, "Fonds euros", None).await;
        security(&pool, "Toyota", Some("7203.T")).await;
        // Has prices in euros: its quotes in dollars are refused.
        let apple = security(&pool, "Apple", Some("AAPL")).await;
        set_manual(&pool, apple, date("2026-10-01"), 21_000)
            .await
            .unwrap();

        assert!(matches!(
            update(&pool, &source, today).await,
            Err(AppError::Price(PriceError::Disabled))
        ));
        assert!(matches!(
            search_symbols(&pool, &source, "FR0010315770").await,
            Err(AppError::Price(PriceError::Disabled))
        ));
        assert!(!update_due(&pool, today).await.unwrap());

        set_updates_enabled(&pool, true).await.unwrap();
        assert!(update_due(&pool, today).await.unwrap());
        set_manual(&pool, world, date("2026-10-20"), 50_000)
            .await
            .unwrap();
        let result = update(&pool, &source, today).await.unwrap();
        // Two securities, and the rate of the pound taken by the S&P 500.
        assert_eq!(result.updated, 3);
        let failures: Vec<(&str, &PriceError)> = result
            .failures
            .iter()
            .map(|f| (f.name.as_str(), &f.error))
            .collect();
        assert_eq!(
            failures,
            [
                (
                    "Apple",
                    &PriceError::CurrencyMismatch {
                        symbol: "AAPL".to_owned(),
                        currency: "USD".to_owned(),
                        expected: "EUR".to_owned()
                    }
                ),
                ("Délisté", &PriceError::UnknownSymbol("OLD.PA".to_owned())),
                (
                    "Toyota",
                    &PriceError::UnsupportedCurrency {
                        symbol: "7203.T".to_owned(),
                        currency: "JPY".to_owned()
                    }
                ),
            ]
        );
        let sp500_security = securities::list(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|s| s.id == sp500)
            .unwrap();
        assert_eq!(sp500_security.currency, "GBP");
        assert_eq!(list(&pool, sp500).await.unwrap()[0].price_cents, 9_016);
        let rates = crate::currency::list_rates(&pool, "GBP").await.unwrap();
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].rate_nano, 1_150_000_000);
        let prices: Vec<(NaiveDate, i64, PriceSourceKind)> = list(&pool, world)
            .await
            .unwrap()
            .into_iter()
            .map(|p| (p.date, p.price_cents, p.source))
            .collect();
        assert_eq!(
            prices,
            [
                (date("2026-10-20"), 50_000, PriceSourceKind::Manual),
                (date("2026-10-19"), 51_225, PriceSourceKind::Yahoo),
            ]
        );
        assert_eq!(settings(&pool).await.unwrap().updated_on, Some(today));
        assert!(!update_due(&pool, today).await.unwrap());

        // A year for a security without prices, a month afterwards.
        update(&pool, &source, today).await.unwrap();
        let asked = source.asked.lock().unwrap().clone();
        assert!(asked.contains(&("CW8.PA".to_owned(), HistoryRange::Month)));
        assert!(asked.contains(&("VUSA.L".to_owned(), HistoryRange::Year)));
        assert!(asked.contains(&("GBPEUR=X".to_owned(), HistoryRange::Year)));
        assert!(asked.contains(&("GBPEUR=X".to_owned(), HistoryRange::Month)));
        assert_eq!(asked.iter().filter(|(s, _)| s == "CW8.PA").count(), 2);
    });
}

#[test]
fn manual_prices_are_positive_and_replace_fetched_ones() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let world = security(&pool, "MSCI World", None).await;
        assert!(matches!(
            set_manual(&pool, world, date("2026-10-20"), 0).await,
            Err(AppError::Price(PriceError::InvalidPrice))
        ));
        assert!(matches!(
            set_manual(&pool, 99, date("2026-10-20"), 100).await,
            Err(AppError::Investment(InvestmentError::SecurityNotFound(99)))
        ));
        set_manual(&pool, world, date("2026-10-20"), 100)
            .await
            .unwrap();
        set_manual(&pool, world, date("2026-10-20"), 200)
            .await
            .unwrap();
        assert_eq!(list(&pool, world).await.unwrap()[0].price_cents, 200);
        delete(&pool, world, date("2026-10-20")).await.unwrap();
        assert!(list(&pool, world).await.unwrap().is_empty());
    });
}
