//! Net worth at a date D (README, calculation rules):
//! - balances of the checking, savings and deferred debit card accounts at D (a card owes
//!   what was spent until it is debited, so its negative balance is counted);
//! - plus the value of each investment account: from its first transaction on, the market
//!   value of its portfolio (last price on or before D, or the cost without a price);
//!   before, its last valuation on or before D;
//! - plus the last valuation on or before D of each asset;
//! - minus the last outstanding capital on or before D of each liability.
//!
//! Archived accounts are left out, as in the other statistics. An account counts from its
//! opening date; a valued account counts 0 until its first valuation.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{BalanceStep, check_period, step_ends};
use crate::account::AccountType;
use crate::error::AppResult;
use crate::investment::Portfolios;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NetWorth {
    pub date: NaiveDate,
    /// Cash + investments + assets − liabilities.
    pub net_worth_cents: i64,
    /// Checking, savings and deferred debit card balances.
    pub cash_cents: i64,
    pub investments_cents: i64,
    pub assets_cents: i64,
    /// Outstanding capital of the debts, as a positive amount.
    pub liabilities_cents: i64,
}

/// (date, value) of the valuations of an account, sorted by date.
type Valuations = Vec<(NaiveDate, i64)>;

/// Account data the computation needs, loaded once.
struct Data {
    /// Changes of the cash balance by date: account openings and operations.
    cash_changes: BTreeMap<NaiveDate, i64>,
    /// Valuations by account: id, type and (date, value) sorted by date.
    valuations: Vec<(i64, AccountType, Valuations)>,
    /// Non-archived investment accounts.
    investment_accounts: Vec<i64>,
    portfolios: Portfolios,
}

async fn load(pool: &SqlitePool, to: NaiveDate) -> AppResult<Data> {
    let mut cash_changes: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    let openings = sqlx::query!(
        r#"SELECT opening_date AS "date!: NaiveDate", opening_balance_cents AS "amount!: i64"
           FROM account
           WHERE archived = 0 AND type IN ('CHECKING', 'SAVINGS', 'CREDIT_CARD')"#
    )
    .fetch_all(pool)
    .await?;
    for opening in openings {
        *cash_changes.entry(opening.date).or_default() += opening.amount;
    }
    let movements = sqlx::query!(
        r#"SELECT o.date AS "date!: NaiveDate", SUM(o.amount_cents) AS "amount!: i64"
           FROM operation o
           JOIN account a ON a.id = o.account_id
           WHERE a.archived = 0 AND a.type IN ('CHECKING', 'SAVINGS', 'CREDIT_CARD')
             AND o.date <= ?1
           GROUP BY o.date"#,
        to
    )
    .fetch_all(pool)
    .await?;
    for movement in movements {
        *cash_changes.entry(movement.date).or_default() += movement.amount;
    }

    let rows = sqlx::query!(
        r#"SELECT v.account_id, a.type AS "account_type: AccountType",
                  v.date AS "date!: NaiveDate", v.value_cents
           FROM valuation v
           JOIN account a ON a.id = v.account_id
           WHERE a.archived = 0 AND v.date <= ?1
           ORDER BY v.account_id, v.date"#,
        to
    )
    .fetch_all(pool)
    .await?;
    let mut by_account: BTreeMap<i64, (AccountType, Vec<(NaiveDate, i64)>)> = BTreeMap::new();
    for row in rows {
        by_account
            .entry(row.account_id)
            .or_insert_with(|| (row.account_type, Vec::new()))
            .1
            .push((row.date, row.value_cents));
    }
    let investment_accounts = sqlx::query_scalar!(
        r#"SELECT id AS "id!" FROM account WHERE archived = 0 AND type = 'INVESTMENT'"#
    )
    .fetch_all(pool)
    .await?;
    Ok(Data {
        cash_changes,
        valuations: by_account
            .into_iter()
            .map(|(id, (account_type, values))| (id, account_type, values))
            .collect(),
        investment_accounts,
        portfolios: Portfolios::load(pool, to).await?,
    })
}

impl Data {
    fn at(&self, date: NaiveDate) -> NetWorth {
        let cash_cents: i64 = self
            .cash_changes
            .range(..=date)
            .map(|(_, amount)| amount)
            .sum();
        let (mut investments_cents, mut assets_cents, mut liabilities_cents) = (0, 0, 0);
        for (account_id, account_type, valuations) in &self.valuations {
            if self.portfolios.values(*account_id, date) {
                continue;
            }
            // Last valuation on or before the date.
            let last = valuations
                .iter()
                .take_while(|(day, _)| *day <= date)
                .last()
                .map_or(0, |&(_, value)| value);
            match account_type {
                AccountType::Investment => investments_cents += last,
                AccountType::Asset => assets_cents += last,
                AccountType::Liability => liabilities_cents += last,
                _ => {}
            }
        }
        for &account_id in &self.investment_accounts {
            if self.portfolios.values(account_id, date) {
                investments_cents += self.portfolios.value(account_id, date).0;
            }
        }
        NetWorth {
            date,
            net_worth_cents: cash_cents + investments_cents + assets_cents - liabilities_cents,
            cash_cents,
            investments_cents,
            assets_cents,
            liabilities_cents,
        }
    }
}

/// Net worth at the end of `date`, with its breakdown.
pub async fn net_worth_at(pool: &SqlitePool, date: NaiveDate) -> AppResult<NetWorth> {
    Ok(load(pool, date).await?.at(date))
}

/// Net worth at the end of each day, week or month between `from` and `to`; `to` is always
/// the last point.
pub async fn net_worth_history(
    pool: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    step: BalanceStep,
) -> AppResult<Vec<NetWorth>> {
    check_period(from, to)?;
    let data = load(pool, to).await?;
    Ok(step_ends(from, to, step)
        .into_iter()
        .map(|date| data.at(date))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::valuation::{self, ValuationInput};
    use crate::account::{self, NewAccount};
    use crate::db::tests::memory_pool;
    use crate::operation::{self, NewOperation};

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    async fn new_account(pool: &SqlitePool, account_type: AccountType, opening: i64) -> i64 {
        account::create(
            pool,
            NewAccount {
                name: format!("{account_type:?}"),
                account_type,
                institution: None,
                opening_balance_cents: opening,
                opening_date: date("2026-01-01"),
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn value(pool: &SqlitePool, account_id: i64, day: &str, value_cents: i64) {
        valuation::create(
            pool,
            account_id,
            ValuationInput {
                date: date(day),
                value_cents,
            },
        )
        .await
        .unwrap();
    }

    async fn spend(pool: &SqlitePool, account_id: i64, day: &str, amount_cents: i64) {
        operation::create(
            pool,
            NewOperation {
                account_id,
                date: date(day),
                value_date: None,
                amount_cents,
                label: "Op".to_owned(),
                category_id: None,
                note: None,
                tags: Vec::new(),
            },
        )
        .await
        .unwrap();
    }

    #[test]
    fn adds_balances_and_valuations_and_subtracts_debts() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let checking = new_account(&pool, AccountType::Checking, 200_000).await;
            new_account(&pool, AccountType::Savings, 1_500_000).await;
            let card = new_account(&pool, AccountType::CreditCard, 0).await;
            let pea = new_account(&pool, AccountType::Investment, 0).await;
            let flat = new_account(&pool, AccountType::Asset, 0).await;
            let loan = new_account(&pool, AccountType::Liability, 0).await;
            let archived = new_account(&pool, AccountType::Asset, 0).await;

            spend(&pool, checking, "2026-09-10", -50_000).await;
            // Card purchases not yet debited from the checking account.
            spend(&pool, card, "2026-09-20", -30_000).await;
            value(&pool, pea, "2026-06-30", 4_000_000).await;
            value(&pool, pea, "2026-09-30", 4_182_000).await;
            value(&pool, flat, "2026-01-15", 21_000_000).await;
            value(&pool, loan, "2026-09-05", 12_699_000).await;
            value(&pool, archived, "2026-01-15", 99_999_900).await;
            account::set_archived(&pool, archived, true).await.unwrap();

            let worth = net_worth_at(&pool, date("2026-09-30")).await.unwrap();
            assert_eq!(
                worth,
                NetWorth {
                    date: date("2026-09-30"),
                    cash_cents: 200_000 - 50_000 + 1_500_000 - 30_000,
                    investments_cents: 4_182_000,
                    assets_cents: 21_000_000,
                    liabilities_cents: 12_699_000,
                    net_worth_cents: 1_620_000 + 4_182_000 + 21_000_000 - 12_699_000,
                }
            );

            // Before the loan valuation and the last PEA valuation.
            let august = net_worth_at(&pool, date("2026-08-31")).await.unwrap();
            assert_eq!(august.investments_cents, 4_000_000);
            assert_eq!(august.liabilities_cents, 0);
            // Before the accounts open, nothing counts.
            let before = net_worth_at(&pool, date("2025-12-31")).await.unwrap();
            assert_eq!(before.net_worth_cents, 0);
        });
    }

    #[test]
    fn history_steps_through_the_period() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            new_account(&pool, AccountType::Checking, 100_000).await;
            let pea = new_account(&pool, AccountType::Investment, 0).await;
            value(&pool, pea, "2026-02-15", 50_000).await;

            let history = net_worth_history(
                &pool,
                date("2026-01-01"),
                date("2026-03-10"),
                BalanceStep::Month,
            )
            .await
            .unwrap();
            let points: Vec<(NaiveDate, i64)> = history
                .iter()
                .map(|w| (w.date, w.net_worth_cents))
                .collect();
            assert_eq!(
                points,
                vec![
                    (date("2026-01-31"), 100_000),
                    (date("2026-02-28"), 150_000),
                    (date("2026-03-10"), 150_000),
                ]
            );
            assert!(
                net_worth_history(
                    &pool,
                    date("2026-02-01"),
                    date("2026-01-01"),
                    BalanceStep::Day
                )
                .await
                .is_err()
            );
        });
    }
}
