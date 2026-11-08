use chrono::NaiveDate;
use sqlx::SqlitePool;

use super::position::{Holding, xirr};
use super::security::is_valid_isin;
use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::{AppError, AppResult};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

async fn new_account(pool: &SqlitePool, account_type: AccountType) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: "PEA".to_owned(),
            account_type,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: Some(date("2026-01-01")),
        },
    )
    .await
    .unwrap()
    .id
}

async fn new_security(pool: &SqlitePool, name: &str, isin: Option<&str>) -> i64 {
    securities::create(
        pool,
        SecurityInput {
            name: name.to_owned(),
            isin: isin.map(str::to_owned),
            symbol: None,
            currency: "EUR".to_owned(),
        },
    )
    .await
    .unwrap()
    .id
}

fn input(
    security_id: i64,
    day: &str,
    kind: TransactionKind,
    quantity_micro: i64,
    amount_cents: i64,
    fees_cents: i64,
) -> TransactionInput {
    TransactionInput {
        security_id,
        date: date(day),
        kind,
        quantity_micro,
        amount_cents,
        fees_cents,
        note: None,
    }
}

fn units(whole: i64) -> i64 {
    whole * QUANTITY_SCALE
}

#[test]
fn validates_isins_with_their_check_digit() {
    assert!(is_valid_isin("US0378331005"));
    assert!(is_valid_isin("FR0000120271"));
    assert!(is_valid_isin("IE00B4L5Y983"));
    assert!(!is_valid_isin("US0378331006"));
    assert!(!is_valid_isin("FR000012027"));
    assert!(!is_valid_isin("120000120271"));
}

#[test]
fn manages_securities() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let world = securities::create(
            &pool,
            SecurityInput {
                name: "  iShares MSCI World ".to_owned(),
                isin: Some(" ie00 b4l5y983 ".to_owned()),
                symbol: Some(" iwda.as ".to_owned()),
                currency: "EUR".to_owned(),
            },
        )
        .await
        .unwrap();
        assert_eq!(world.name, "iShares MSCI World");
        assert_eq!(world.isin.as_deref(), Some("IE00B4L5Y983"));
        assert_eq!(world.symbol.as_deref(), Some("IWDA.AS"));

        let taken = securities::create(
            &pool,
            SecurityInput {
                name: "Autre".to_owned(),
                isin: Some("IE00B4L5Y983".to_owned()),
                symbol: None,
                currency: "EUR".to_owned(),
            },
        )
        .await;
        assert!(matches!(
            taken,
            Err(AppError::Investment(InvestmentError::IsinTaken(_)))
        ));
        let invalid = securities::create(
            &pool,
            SecurityInput {
                name: "Autre".to_owned(),
                isin: Some("IE00B4L5Y984".to_owned()),
                symbol: None,
                currency: "EUR".to_owned(),
            },
        )
        .await;
        assert!(matches!(
            invalid,
            Err(AppError::Investment(InvestmentError::InvalidIsin(_)))
        ));
        // Without ISIN.
        let fund = new_security(&pool, "Fonds euros", Some("")).await;

        let account_id = new_account(&pool, AccountType::Investment).await;
        transactions::create(
            &pool,
            account_id,
            input(
                world.id,
                "2026-02-01",
                TransactionKind::Buy,
                units(1),
                10_000,
                0,
            ),
        )
        .await
        .unwrap();
        assert!(matches!(
            securities::delete(&pool, world.id).await,
            Err(AppError::Investment(InvestmentError::SecurityInUse))
        ));
        securities::delete(&pool, fund).await.unwrap();
        let names: Vec<(String, i64)> = securities::list(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|s| (s.name, s.transaction_count))
            .collect();
        assert_eq!(names, [("iShares MSCI World".to_owned(), 1)]);
    });
}

#[test]
fn keeps_the_weighted_average_cost_with_buying_fees() {
    let holding = Holding::default()
        // 10 units for 1 000 € and 1 € of fees.
        .apply(TransactionKind::Buy, units(10), 100_000, 100)
        .unwrap();
    assert_eq!(holding.unit_cost_cents(), 10_010);
    // 2.5 units for 300 €, no fees.
    let holding = holding
        .apply(TransactionKind::Buy, 2_500_000, 30_000, 0)
        .unwrap();
    assert_eq!(holding.quantity_micro, 12_500_000);
    assert_eq!(holding.cost_cents, 130_100);
    assert_eq!(holding.unit_cost_cents(), 10_408);
    // Selling keeps the unit cost; its price and fees do not change the cost.
    let holding = holding
        .apply(TransactionKind::Sell, 2_500_000, 99_999, 500)
        .unwrap();
    assert_eq!(holding.quantity_micro, units(10));
    assert_eq!(holding.cost_cents, 104_080);
    assert_eq!(holding.unit_cost_cents(), 10_408);
    // Proceeds net of fees − cost of the units sold (130 100 − 104 080).
    assert_eq!(holding.realized_cents, 99_999 - 500 - 26_020);
    let holding = holding
        .apply(TransactionKind::Dividend, 0, 1_434, 200)
        .unwrap();
    assert_eq!(holding.dividends_cents, 1_234);
    assert_eq!(holding.cost_cents, 104_080);
    assert!(
        holding
            .apply(TransactionKind::Sell, units(10) + 1, 1, 0)
            .is_none()
    );
    let sold = holding
        .apply(TransactionKind::Sell, units(10), 1, 0)
        .unwrap();
    assert_eq!((sold.quantity_micro, sold.cost_cents), (0, 0));
}

#[test]
fn computes_the_positions_at_a_date() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = new_account(&pool, AccountType::Investment).await;
        let world = new_security(&pool, "MSCI World", Some("IE00B4L5Y983")).await;
        let total = new_security(&pool, "TotalEnergies", Some("FR0000120271")).await;
        let sold = new_security(&pool, "Apple", Some("US0378331005")).await;
        for transaction in [
            input(
                world,
                "2026-02-01",
                TransactionKind::Buy,
                units(10),
                100_000,
                100,
            ),
            input(
                total,
                "2026-02-03",
                TransactionKind::Buy,
                units(20),
                110_000,
                200,
            ),
            input(
                sold,
                "2026-02-04",
                TransactionKind::Buy,
                units(1),
                20_000,
                0,
            ),
            input(
                sold,
                "2026-03-04",
                TransactionKind::Sell,
                units(1),
                22_000,
                0,
            ),
            input(total, "2026-03-10", TransactionKind::Dividend, 0, 1_500, 0),
            input(
                world,
                "2026-04-01",
                TransactionKind::Buy,
                2_500_000,
                30_000,
                0,
            ),
        ] {
            transactions::create(&pool, account_id, transaction)
                .await
                .unwrap();
        }

        let at = |day: &'static str| {
            let pool = pool.clone();
            async move {
                positions(&pool, account_id, date(day))
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|p| {
                        (
                            p.security_name,
                            p.quantity_micro,
                            p.cost_cents,
                            p.dividends_cents,
                        )
                    })
                    .collect::<Vec<_>>()
            }
        };
        assert_eq!(
            at("2026-03-31").await,
            [
                ("MSCI World".to_owned(), units(10), 100_100, 0),
                ("TotalEnergies".to_owned(), units(20), 110_200, 1_500),
            ]
        );
        assert_eq!(at("2026-04-01").await[0].1, 12_500_000);

        prices::set_manual(&pool, world, date("2026-03-15"), 10_500)
            .await
            .unwrap();
        prices::set_manual(&pool, world, date("2026-05-01"), 11_000)
            .await
            .unwrap();
        let held = positions(&pool, account_id, date("2026-04-30"))
            .await
            .unwrap();
        assert_eq!(
            held[0].last_price.map(|p| (p.date, p.price_cents)),
            Some((date("2026-03-15"), 10_500))
        );
        assert_eq!(held[1].last_price, None);
        assert_eq!(
            transactions::list(&pool, account_id).await.unwrap()[0].date,
            date("2026-04-01")
        );
    });
}

#[test]
fn refuses_to_sell_more_than_held() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = new_account(&pool, AccountType::Investment).await;
        let world = new_security(&pool, "MSCI World", None).await;
        let buy = transactions::create(
            &pool,
            account_id,
            input(
                world,
                "2026-02-01",
                TransactionKind::Buy,
                units(10),
                100_000,
                0,
            ),
        )
        .await
        .unwrap();
        let oversold = |result: AppResult<InvestmentTransaction>| {
            matches!(
                result,
                Err(AppError::Investment(InvestmentError::Oversold { .. }))
            )
        };
        assert!(oversold(
            transactions::create(
                &pool,
                account_id,
                input(world, "2026-03-01", TransactionKind::Sell, units(11), 1, 0),
            )
            .await
        ));
        // Before the buy.
        assert!(oversold(
            transactions::create(
                &pool,
                account_id,
                input(world, "2026-01-15", TransactionKind::Sell, units(1), 1, 0),
            )
            .await
        ));
        let sell = transactions::create(
            &pool,
            account_id,
            input(
                world,
                "2026-03-01",
                TransactionKind::Sell,
                units(4),
                50_000,
                0,
            ),
        )
        .await
        .unwrap();
        // Moving the buy after the sell, or deleting it, leaves the sell uncovered.
        assert!(oversold(
            transactions::update(
                &pool,
                buy.id,
                input(
                    world,
                    "2026-03-02",
                    TransactionKind::Buy,
                    units(10),
                    100_000,
                    0
                ),
            )
            .await
        ));
        assert!(matches!(
            transactions::delete(&pool, buy.id).await,
            Err(AppError::Investment(InvestmentError::Oversold { .. }))
        ));
        transactions::delete(&pool, sell.id).await.unwrap();
        transactions::delete(&pool, buy.id).await.unwrap();
        assert!(
            transactions::list(&pool, account_id)
                .await
                .unwrap()
                .is_empty()
        );
    });
}

#[test]
fn checks_the_account_and_the_fields() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let checking = new_account(&pool, AccountType::Checking).await;
        let account_id = new_account(&pool, AccountType::Investment).await;
        let world = new_security(&pool, "MSCI World", None).await;
        let error = |result: AppResult<InvestmentTransaction>| match result {
            Err(AppError::Investment(error)) => error,
            other => panic!("unexpected {other:?}"),
        };
        let buy = input(world, "2026-02-01", TransactionKind::Buy, units(1), 100, 0);
        assert!(matches!(
            error(transactions::create(&pool, checking, buy.clone()).await),
            InvestmentError::NotAnInvestmentAccount
        ));
        assert!(matches!(
            error(
                transactions::create(
                    &pool,
                    account_id,
                    TransactionInput {
                        date: date("2025-12-31"),
                        ..buy.clone()
                    }
                )
                .await
            ),
            InvestmentError::DateBeforeOpening(_)
        ));
        assert!(matches!(
            error(
                transactions::create(
                    &pool,
                    account_id,
                    TransactionInput {
                        quantity_micro: 0,
                        ..buy.clone()
                    }
                )
                .await
            ),
            InvestmentError::MissingQuantity
        ));
        assert!(matches!(
            error(
                transactions::create(
                    &pool,
                    account_id,
                    TransactionInput {
                        kind: TransactionKind::Dividend,
                        ..buy.clone()
                    }
                )
                .await
            ),
            InvestmentError::QuantityOnDividend
        ));
        assert!(matches!(
            error(
                transactions::create(
                    &pool,
                    account_id,
                    TransactionInput {
                        amount_cents: 0,
                        ..buy.clone()
                    }
                )
                .await
            ),
            InvestmentError::ZeroAmount
        ));
        assert!(matches!(
            error(
                transactions::create(
                    &pool,
                    account_id,
                    TransactionInput {
                        security_id: 99,
                        ..buy
                    }
                )
                .await
            ),
            InvestmentError::SecurityNotFound(99)
        ));
    });
}

#[test]
fn computes_the_annual_return_of_dated_flows() {
    let rate = xirr(&[(date("2025-01-01"), -10_000), (date("2026-01-01"), 11_000)]).unwrap();
    assert!((rate - 0.10).abs() < 1e-6, "{rate}");
    // Half the money a year later: same 10 % on each euro.
    let rate = xirr(&[
        (date("2025-01-01"), -10_000),
        (date("2026-01-01"), -10_000),
        (date("2027-01-01"), 12_100 + 11_000),
    ])
    .unwrap();
    assert!((rate - 0.10).abs() < 1e-4, "{rate}");
    assert_eq!(xirr(&[(date("2025-01-01"), 100)]), None);
}

#[test]
fn reports_the_gains_of_an_account() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = new_account(&pool, AccountType::Investment).await;
        let world = new_security(&pool, "MSCI World", None).await;
        let total = new_security(&pool, "TotalEnergies", None).await;
        for transaction in [
            input(
                world,
                "2026-01-10",
                TransactionKind::Buy,
                units(10),
                100_000,
                0,
            ),
            input(
                total,
                "2026-02-10",
                TransactionKind::Buy,
                units(20),
                100_000,
                0,
            ),
            input(total, "2026-06-10", TransactionKind::Dividend, 0, 3_000, 0),
            input(
                total,
                "2027-03-10",
                TransactionKind::Sell,
                units(10),
                60_000,
                100,
            ),
        ] {
            transactions::create(&pool, account_id, transaction)
                .await
                .unwrap();
        }
        prices::set_manual(&pool, world, date("2027-03-31"), 12_000)
            .await
            .unwrap();

        let result = performance(&pool, account_id, date("2027-04-01"))
            .await
            .unwrap();
        // World 1 200 € for 1 000 €; Total without price, at its cost of 500 €.
        assert_eq!(result.cost_cents, 150_000);
        assert_eq!(result.market_value_cents, 170_000);
        assert_eq!(result.unrealized_cents, 20_000);
        assert_eq!(result.realized_cents, 60_000 - 100 - 50_000);
        assert_eq!(result.dividends_cents, 3_000);
        assert_eq!(result.total_gain_cents, 20_000 + 9_900 + 3_000);
        assert_eq!(result.bought_cents, 200_000);
        assert_eq!(result.total_gain_basis_points, Some(1_645));
        assert!(result.annualized_basis_points.is_some_and(|bp| bp > 0));
        let years: Vec<(i32, i64, i64)> = result
            .years
            .iter()
            .map(|y| (y.year, y.realized_cents, y.dividends_cents))
            .collect();
        assert_eq!(years, [(2027, 9_900, 0), (2026, 0, 3_000)]);

        // Under a year after the first buy: no annual return.
        let early = performance(&pool, account_id, date("2026-12-31"))
            .await
            .unwrap();
        assert_eq!(early.annualized_basis_points, None);

        let sells: Vec<Option<i64>> = transactions::list(&pool, account_id)
            .await
            .unwrap()
            .iter()
            .map(|t| t.realized_cents)
            .collect();
        assert_eq!(sells, [Some(9_900), None, None, None]);

        let held = positions(&pool, account_id, date("2027-04-01"))
            .await
            .unwrap();
        assert_eq!(
            held.iter()
                .map(|p| (p.market_value_cents, p.unrealized_cents))
                .collect::<Vec<_>>(),
            [(120_000, 20_000), (50_000, 0)]
        );
    });
}

#[test]
fn values_the_account_by_its_portfolio_from_its_first_transaction() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = new_account(&pool, AccountType::Investment).await;
        let world = new_security(&pool, "MSCI World", None).await;
        crate::account::valuation::create(
            &pool,
            account_id,
            crate::account::valuation::ValuationInput {
                date: date("2026-01-31"),
                value_cents: 90_000,
            },
        )
        .await
        .unwrap();
        transactions::create(
            &pool,
            account_id,
            input(
                world,
                "2026-02-10",
                TransactionKind::Buy,
                units(10),
                100_000,
                0,
            ),
        )
        .await
        .unwrap();
        prices::set_manual(&pool, world, date("2026-03-01"), 11_000)
            .await
            .unwrap();

        let investments = |day: &'static str| {
            let pool = pool.clone();
            async move {
                crate::stats::net_worth_at(&pool, date(day))
                    .await
                    .unwrap()
                    .investments_cents
            }
        };
        // The valuation before the first transaction, then the cost, then the price.
        assert_eq!(investments("2026-02-01").await, 90_000);
        assert_eq!(investments("2026-02-15").await, 100_000);
        assert_eq!(investments("2026-03-15").await, 110_000);

        let summaries = account::list_summaries(&pool, false, date("2026-03-15"))
            .await
            .unwrap();
        assert_eq!(summaries[0].balance_cents, Some(110_000));
        assert_eq!(summaries[0].valued_on, Some(date("2026-03-01")));
    });
}

#[test]
fn values_a_security_quoted_in_another_currency_in_the_currency_of_the_account() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = new_account(&pool, AccountType::Investment).await;
        let apple = securities::create(
            &pool,
            SecurityInput {
                name: "Apple".to_owned(),
                isin: None,
                symbol: None,
                currency: "USD".to_owned(),
            },
        )
        .await
        .unwrap()
        .id;
        transactions::create(
            &pool,
            account_id,
            input(
                apple,
                "2026-03-02",
                TransactionKind::Buy,
                units(10),
                180_000,
                0,
            ),
        )
        .await
        .unwrap();
        prices::set_manual(&pool, apple, date("2026-10-20"), 25_000)
            .await
            .unwrap();
        // Without a rate, the cost.
        let held = positions(&pool, account_id, date("2026-10-21"))
            .await
            .unwrap();
        assert_eq!(held[0].market_value_cents, 180_000);
        assert_eq!(held[0].currency, "USD");

        // 10 × 250 $ at 0.90 € on the day of the price.
        crate::currency::set_manual_rate(&pool, "USD", date("2026-10-20"), 900_000_000)
            .await
            .unwrap();
        crate::currency::set_manual_rate(&pool, "USD", date("2026-10-21"), 800_000_000)
            .await
            .unwrap();
        let held = positions(&pool, account_id, date("2026-10-21"))
            .await
            .unwrap();
        assert_eq!(held[0].market_value_cents, 225_000);
        assert_eq!(held[0].unrealized_cents, 45_000);

        // Its currency is fixed once it has prices.
        assert!(matches!(
            securities::update(
                &pool,
                apple,
                SecurityInput {
                    name: "Apple".to_owned(),
                    isin: None,
                    symbol: None,
                    currency: "EUR".to_owned(),
                },
            )
            .await,
            Err(AppError::Currency(
                crate::currency::CurrencyError::SecurityCurrencyLocked
            ))
        ));
    });
}
