use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{QUANTITY_SCALE, TransactionKind};
use crate::amount::mul_div;
use crate::currency::{REFERENCE, Rates};
use crate::error::AppResult;

/// A security held in an account at a date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub security_id: i64,
    pub security_name: String,
    pub isin: Option<String>,
    /// Millionths of a unit.
    pub quantity_micro: i64,
    /// Weighted average cost of the units held, buying fees included.
    pub cost_cents: i64,
    /// Cost of one unit, rounded to the cent.
    pub unit_cost_cents: i64,
    /// Dividends received since the first buy, net of their fees.
    pub dividends_cents: i64,
    /// Last known price on or before the date, in the currency of the security.
    pub last_price: Option<PricePoint>,
    /// Currency of the prices of the security.
    pub currency: String,
    /// Quantity × last price, converted to the currency of the account at the rate of the day
    /// of the price; the cost without a price or a rate.
    pub market_value_cents: i64,
    /// Market value − cost; 0 without a price.
    pub unrealized_cents: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PricePoint {
    pub date: NaiveDate,
    pub price_cents: i64,
}

/// Gains of an investment account at a date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Performance {
    /// Cost of the units held.
    pub cost_cents: i64,
    pub market_value_cents: i64,
    pub unrealized_cents: i64,
    /// Gains of the sells: proceeds net of fees − cost of the units sold.
    pub realized_cents: i64,
    /// Dividends net of their fees.
    pub dividends_cents: i64,
    /// Unrealized + realized + dividends.
    pub total_gain_cents: i64,
    /// Every buy, fees included: the base of the percentage.
    pub bought_cents: i64,
    /// Total gain ÷ bought, in hundredths of a percent; `None` without buys.
    pub total_gain_basis_points: Option<i64>,
    /// Money-weighted annual return (XIRR) of the buys, sells, dividends and market value, in
    /// hundredths of a percent; `None` under a year after the first buy.
    pub annualized_basis_points: Option<i64>,
    /// Realized gains and dividends by calendar year, the most recent first.
    pub years: Vec<YearGains>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct YearGains {
    pub year: i32,
    pub realized_cents: i64,
    pub dividends_cents: i64,
}

/// Quantity, cost, realized gains and dividends of a security, transaction after transaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Holding {
    pub quantity_micro: i64,
    pub cost_cents: i64,
    pub realized_cents: i64,
    pub dividends_cents: i64,
}

impl Holding {
    /// Applies a transaction; `None` when it sells more than held.
    pub fn apply(
        self,
        kind: TransactionKind,
        quantity_micro: i64,
        amount_cents: i64,
        fees_cents: i64,
    ) -> Option<Holding> {
        Some(match kind {
            TransactionKind::Buy => Holding {
                quantity_micro: self.quantity_micro + quantity_micro,
                cost_cents: self.cost_cents + amount_cents + fees_cents,
                ..self
            },
            TransactionKind::Sell => {
                if quantity_micro > self.quantity_micro {
                    return None;
                }
                let remaining = self.quantity_micro - quantity_micro;
                let cost_cents = mul_div(self.cost_cents, remaining, self.quantity_micro);
                let sold_cost = self.cost_cents - cost_cents;
                Holding {
                    quantity_micro: remaining,
                    cost_cents,
                    realized_cents: self.realized_cents + amount_cents - fees_cents - sold_cost,
                    ..self
                }
            }
            TransactionKind::Dividend => Holding {
                dividends_cents: self.dividends_cents + amount_cents - fees_cents,
                ..self
            },
        })
    }

    pub fn unit_cost_cents(&self) -> i64 {
        if self.quantity_micro == 0 {
            0
        } else {
            mul_div(self.cost_cents, QUANTITY_SCALE, self.quantity_micro)
        }
    }

    /// Quantity × price, in the currency of the price.
    pub fn market_value_cents(&self, price: PricePoint) -> i64 {
        mul_div(price.price_cents, self.quantity_micro, QUANTITY_SCALE)
    }
}

#[derive(Debug, Clone)]
struct Transaction {
    date: NaiveDate,
    security_id: i64,
    kind: TransactionKind,
    quantity_micro: i64,
    amount_cents: i64,
    fees_cents: i64,
}

/// Transactions and prices of the portfolios, loaded once to value them at many dates.
/// Amounts are in the currency of the account, prices in the currency of the security.
pub struct Portfolios {
    /// Transactions of each account, by date.
    transactions: HashMap<i64, Vec<Transaction>>,
    /// Prices of each security, by date.
    prices: HashMap<i64, Vec<PricePoint>>,
    account_currencies: HashMap<i64, String>,
    security_currencies: HashMap<i64, String>,
    rates: Rates,
}

impl Portfolios {
    /// Transactions and prices up to `to`.
    pub async fn load(pool: &SqlitePool, to: NaiveDate) -> AppResult<Portfolios> {
        let rows = sqlx::query!(
            r#"SELECT account_id, security_id, date AS "date: NaiveDate",
                      kind AS "kind: TransactionKind", quantity_micro, amount_cents, fees_cents
               FROM investment_transaction
               WHERE date <= ?1
               ORDER BY date, id"#,
            to
        )
        .fetch_all(pool)
        .await?;
        let mut transactions: HashMap<i64, Vec<Transaction>> = HashMap::new();
        for row in rows {
            transactions
                .entry(row.account_id)
                .or_default()
                .push(Transaction {
                    date: row.date,
                    security_id: row.security_id,
                    kind: row.kind,
                    quantity_micro: row.quantity_micro,
                    amount_cents: row.amount_cents,
                    fees_cents: row.fees_cents,
                });
        }
        let rows = sqlx::query!(
            r#"SELECT security_id, date AS "date: NaiveDate", price_cents
               FROM security_price
               WHERE date <= ?1
               ORDER BY security_id, date"#,
            to
        )
        .fetch_all(pool)
        .await?;
        let mut prices: HashMap<i64, Vec<PricePoint>> = HashMap::new();
        for row in rows {
            prices.entry(row.security_id).or_default().push(PricePoint {
                date: row.date,
                price_cents: row.price_cents,
            });
        }
        let account_currencies = sqlx::query!(r#"SELECT id AS "id!", currency FROM account"#)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| (row.id, row.currency))
            .collect();
        let security_currencies = sqlx::query!(r#"SELECT id AS "id!", currency FROM security"#)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| (row.id, row.currency))
            .collect();
        Ok(Portfolios {
            transactions,
            prices,
            account_currencies,
            security_currencies,
            rates: Rates::load(pool).await?,
        })
    }

    /// Whether the account is valued by its portfolio at `date`: from its first transaction
    /// on. Before, its valuations count.
    pub fn values(&self, account_id: i64, date: NaiveDate) -> bool {
        self.transactions
            .get(&account_id)
            .and_then(|t| t.first())
            .is_some_and(|first| first.date <= date)
    }

    /// Holdings of each security of an account at the end of `date`.
    fn holdings(&self, account_id: i64, date: NaiveDate) -> BTreeMap<i64, Holding> {
        let mut holdings: BTreeMap<i64, Holding> = BTreeMap::new();
        for t in self.transactions.get(&account_id).into_iter().flatten() {
            if t.date > date {
                break;
            }
            let holding = holdings.entry(t.security_id).or_default();
            // The transactions are checked on every change: a sell never exceeds the holding.
            *holding = holding
                .apply(t.kind, t.quantity_micro, t.amount_cents, t.fees_cents)
                .unwrap_or_default();
        }
        holdings
    }

    /// Last price of a security on or before `date`.
    fn price(&self, security_id: i64, date: NaiveDate) -> Option<PricePoint> {
        let prices = self.prices.get(&security_id)?;
        let count = prices.partition_point(|p| p.date <= date);
        count.checked_sub(1).map(|index| prices[index])
    }

    fn currency(currencies: &HashMap<i64, String>, id: i64) -> &str {
        currencies.get(&id).map_or(REFERENCE, String::as_str)
    }

    /// Currency of the amounts of an account.
    pub fn account_currency(&self, account_id: i64) -> &str {
        Self::currency(&self.account_currencies, account_id)
    }

    /// Market value of a holding of an account in the currency of the account, converted at
    /// the rate of the day of the price, and the date of that price; the cost without a price
    /// or a rate.
    fn market_value(
        &self,
        account_id: i64,
        security_id: i64,
        holding: &Holding,
        date: NaiveDate,
    ) -> (i64, Option<NaiveDate>) {
        self.price(security_id, date)
            .and_then(|price| {
                let value = self.rates.convert(
                    holding.market_value_cents(price),
                    Self::currency(&self.security_currencies, security_id),
                    self.account_currency(account_id),
                    price.date,
                )?;
                Some((value, Some(price.date)))
            })
            .unwrap_or((holding.cost_cents, None))
    }

    /// Market value of the units held in an account at the end of `date`, in the currency of
    /// the account, and the date of the most recent price used.
    pub fn value(&self, account_id: i64, date: NaiveDate) -> (i64, Option<NaiveDate>) {
        let mut value = 0;
        let mut priced_on: Option<NaiveDate> = None;
        for (security_id, holding) in self.holdings(account_id, date) {
            if holding.quantity_micro == 0 {
                continue;
            }
            let (market_value, price_date) =
                self.market_value(account_id, security_id, &holding, date);
            value += market_value;
            priced_on = priced_on.max(price_date);
        }
        (value, priced_on)
    }

    /// Gains of an account at the end of `date`.
    fn performance(&self, account_id: i64, date: NaiveDate) -> Performance {
        let holdings = self.holdings(account_id, date);
        let (mut cost_cents, mut market_value_cents, mut realized_cents, mut dividends_cents) =
            (0, 0, 0, 0);
        for (&security_id, holding) in &holdings {
            cost_cents += holding.cost_cents;
            market_value_cents += self.market_value(account_id, security_id, holding, date).0;
            realized_cents += holding.realized_cents;
            dividends_cents += holding.dividends_cents;
        }
        let unrealized_cents = market_value_cents - cost_cents;
        let total_gain_cents = unrealized_cents + realized_cents + dividends_cents;

        // Cash flows of the investor: buys out, sells and dividends in, the value at the end.
        let mut flows: Vec<(NaiveDate, i64)> = Vec::new();
        let mut years: BTreeMap<i32, YearGains> = BTreeMap::new();
        let mut running: BTreeMap<i64, Holding> = BTreeMap::new();
        let mut bought_cents = 0;
        for t in self.transactions.get(&account_id).into_iter().flatten() {
            if t.date > date {
                break;
            }
            let holding = running.entry(t.security_id).or_default();
            let after = holding
                .apply(t.kind, t.quantity_micro, t.amount_cents, t.fees_cents)
                .unwrap_or_default();
            let year = years.entry(t.date.year()).or_insert(YearGains {
                year: t.date.year(),
                realized_cents: 0,
                dividends_cents: 0,
            });
            year.realized_cents += after.realized_cents - holding.realized_cents;
            year.dividends_cents += after.dividends_cents - holding.dividends_cents;
            *holding = after;
            match t.kind {
                TransactionKind::Buy => {
                    bought_cents += t.amount_cents + t.fees_cents;
                    flows.push((t.date, -(t.amount_cents + t.fees_cents)));
                }
                TransactionKind::Sell | TransactionKind::Dividend => {
                    flows.push((t.date, t.amount_cents - t.fees_cents));
                }
            }
        }
        flows.push((date, market_value_cents));
        let first_buy = flows.first().map(|(day, _)| *day);
        let annualized_basis_points = first_buy
            .filter(|first| (date - *first).num_days() >= 365)
            .and_then(|_| xirr(&flows))
            .map(|rate| (rate * 10_000.0).round() as i64);

        Performance {
            cost_cents,
            market_value_cents,
            unrealized_cents,
            realized_cents,
            dividends_cents,
            total_gain_cents,
            bought_cents,
            total_gain_basis_points: (bought_cents > 0)
                .then(|| mul_div(total_gain_cents, 10_000, bought_cents)),
            annualized_basis_points,
            years: years
                .into_values()
                .rev()
                .filter(|y| y.realized_cents != 0 || y.dividends_cents != 0)
                .collect(),
        }
    }
}

/// Annual rate that makes the present value of the flows zero (dates in days of 365), by
/// bisection; `None` when there is no such rate between −99.99 % and +1 000 %. A rate, not an
/// amount: computed in floating point.
pub(super) fn xirr(flows: &[(NaiveDate, i64)]) -> Option<f64> {
    let start = flows.first()?.0;
    let npv = |rate: f64| -> f64 {
        flows
            .iter()
            .map(|(date, cents)| {
                let years = (*date - start).num_days() as f64 / 365.0;
                *cents as f64 / (1.0 + rate).powf(years)
            })
            .sum()
    };
    let (mut low, mut high) = (-0.9999, 10.0);
    let (mut npv_low, npv_high) = (npv(low), npv(high));
    if npv_low.signum() == npv_high.signum() {
        return None;
    }
    for _ in 0..200 {
        let middle = (low + high) / 2.0;
        let npv_middle = npv(middle);
        if npv_middle.signum() == npv_low.signum() {
            low = middle;
            npv_low = npv_middle;
        } else {
            high = middle;
        }
    }
    Some((low + high) / 2.0)
}

/// Securities held in an investment account at the end of `date`, by name.
pub async fn positions(
    pool: &SqlitePool,
    account_id: i64,
    date: NaiveDate,
) -> AppResult<Vec<Position>> {
    let portfolios = Portfolios::load(pool, date).await?;
    let securities: HashMap<i64, (String, Option<String>)> =
        sqlx::query!(r#"SELECT id AS "id!", name, isin FROM security"#)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| (row.id, (row.name, row.isin)))
            .collect();
    let mut positions: Vec<Position> = portfolios
        .holdings(account_id, date)
        .into_iter()
        .filter(|(_, holding)| holding.quantity_micro > 0)
        .map(|(security_id, holding)| {
            let (security_name, isin) = securities.get(&security_id).cloned().unwrap_or_default();
            let last_price = portfolios.price(security_id, date);
            let (market_value_cents, _) =
                portfolios.market_value(account_id, security_id, &holding, date);
            Position {
                security_id,
                security_name,
                isin,
                currency: Portfolios::currency(&portfolios.security_currencies, security_id)
                    .to_owned(),
                quantity_micro: holding.quantity_micro,
                cost_cents: holding.cost_cents,
                unit_cost_cents: holding.unit_cost_cents(),
                dividends_cents: holding.dividends_cents,
                last_price,
                market_value_cents,
                unrealized_cents: market_value_cents - holding.cost_cents,
            }
        })
        .collect();
    positions.sort_by_key(|p| p.security_name.to_lowercase());
    Ok(positions)
}

/// Gains of an investment account at the end of `date`.
pub async fn performance(
    pool: &SqlitePool,
    account_id: i64,
    date: NaiveDate,
) -> AppResult<Performance> {
    Ok(Portfolios::load(pool, date)
        .await?
        .performance(account_id, date))
}
