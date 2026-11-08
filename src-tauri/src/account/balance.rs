use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{Account, AccountError, AccountType};
use crate::currency::Rates;
use crate::error::AppResult;
use crate::investment::Portfolios;

/// An account with its balance at a given date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub account: Account,
    /// Balance from the operations; for an investment account, the market value of its
    /// portfolio from its first transaction on, else its last valuation on or before the date
    /// (as for assets); for liabilities, minus the last outstanding capital. `None` for a
    /// valued account without valuation yet. In the currency of the account.
    pub balance_cents: Option<i64>,
    /// The balance in euros at the rate of the date; 0 without a rate.
    pub reference_balance_cents: Option<i64>,
    /// Personal share in euros, 50% for a joint account.
    pub personal_balance_cents: Option<i64>,
    /// Date of the valuation behind the balance of a valued account, or of the most recent
    /// price of its portfolio.
    pub valued_on: Option<NaiveDate>,
}

/// Balance of an account at the end of `date`: opening balance plus every operation dated on or
/// before `date`, internal transfers included.
///
/// Only meaningful for checking, savings and credit card accounts; investments, assets and
/// liabilities are valued through their valuations.
pub async fn balance_at(pool: &SqlitePool, account_id: i64, date: NaiveDate) -> AppResult<i64> {
    let balance = sqlx::query_scalar!(
        r#"SELECT a.opening_balance_cents + COALESCE(
                      (SELECT SUM(o.amount_cents) FROM operation o
                       WHERE o.account_id = a.id AND o.date <= ?2), 0) AS "balance!: i64"
           FROM account a
           WHERE a.id = ?1"#,
        account_id,
        date
    )
    .fetch_optional(pool)
    .await?;
    Ok(balance.ok_or(AccountError::NotFound(account_id))?)
}

/// Accounts (same order as [`super::list`]) with their balance at the end of `date`.
pub async fn list_summaries(
    pool: &SqlitePool,
    include_archived: bool,
    date: NaiveDate,
) -> AppResult<Vec<AccountSummary>> {
    let accounts = super::list(pool, include_archived).await?;
    let sums: HashMap<i64, i64> = sqlx::query!(
        r#"SELECT account_id, SUM(amount_cents) AS "total!: i64"
           FROM operation
           WHERE date <= ?1
           GROUP BY account_id"#,
        date
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| (row.account_id, row.total))
    .collect();

    let valuations: HashMap<i64, (NaiveDate, i64)> = sqlx::query!(
        r#"SELECT v.account_id, v.date AS "date: NaiveDate", v.value_cents
           FROM valuation v
           WHERE v.date = (SELECT MAX(w.date) FROM valuation w
                           WHERE w.account_id = v.account_id AND w.date <= ?1)"#,
        date
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| (row.account_id, (row.date, row.value_cents)))
    .collect();

    let portfolios = Portfolios::load(pool, date).await?;
    let rates = Rates::load(pool).await?;
    Ok(accounts
        .into_iter()
        .map(|account| {
            let (balance_cents, valued_on) = if portfolios.values(account.id, date) {
                let (value, priced_on) = portfolios.value(account.id, date);
                (Some(value), priced_on)
            } else if account.account_type.has_operations() {
                let sum = sums.get(&account.id).copied().unwrap_or(0);
                (Some(account.opening_balance_cents + sum), None)
            } else {
                match valuations.get(&account.id) {
                    Some(&(valued_on, value)) => {
                        let sign = if account.account_type == AccountType::Liability {
                            -1
                        } else {
                            1
                        };
                        (Some(sign * value), Some(valued_on))
                    }
                    None => (None, None),
                }
            };
            let reference_balance_cents =
                balance_cents.map(|balance| rates.to_reference(balance, &account.currency, date));
            AccountSummary {
                personal_balance_cents: reference_balance_cents
                    .map(|balance| super::personal_share(balance, account.is_joint)),
                reference_balance_cents,
                account,
                balance_cents,
                valued_on,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use super::*;
    use crate::account::{self, AccountType, NewAccount};
    use crate::db::tests::memory_pool;
    use crate::error::AppError;

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    async fn checking_account(pool: &SqlitePool, opening_balance_cents: i64) -> i64 {
        account::create(
            pool,
            NewAccount {
                is_joint: false,
                name: "Compte courant".to_owned(),
                account_type: AccountType::Checking,
                currency: "EUR".to_owned(),
                institution: None,
                opening_balance_cents,
                opening_date: Some(date("2026-10-01")),
            },
        )
        .await
        .unwrap()
        .id
    }

    async fn add_operation(pool: &SqlitePool, account_id: i64, day: &str, amount_cents: i64) {
        sqlx::query!(
            "INSERT INTO operation
             (account_id, date, amount_cents, raw_label, normalized_label, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'TEST', 'TEST', '2026-10-20T00:00:00Z', '2026-10-20T00:00:00Z')",
            account_id,
            day,
            amount_cents
        )
        .execute(pool)
        .await
        .unwrap();
    }

    #[test]
    fn is_the_opening_balance_without_operations() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let id = checking_account(&pool, 41323).await;
            assert_eq!(
                balance_at(&pool, id, date("2026-10-19")).await.unwrap(),
                41323
            );
        });
    }

    #[test]
    fn adds_operations_up_to_the_date_included() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let id = checking_account(&pool, 41323).await;
            add_operation(&pool, id, "2026-10-01", 328000).await;
            add_operation(&pool, id, "2026-10-05", -84200).await;
            // Internal transfers count in balances.
            add_operation(&pool, id, "2026-10-15", -30000).await;
            add_operation(&pool, id, "2026-10-19", -6428).await;

            assert_eq!(
                balance_at(&pool, id, date("2026-10-05")).await.unwrap(),
                285123
            );
            assert_eq!(
                balance_at(&pool, id, date("2026-10-18")).await.unwrap(),
                255123
            );
            assert_eq!(
                balance_at(&pool, id, date("2026-10-19")).await.unwrap(),
                248695
            );
        });
    }

    #[test]
    fn ignores_other_accounts() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let id = checking_account(&pool, 1000).await;
            let other = checking_account(&pool, 0).await;
            add_operation(&pool, other, "2026-10-02", -500).await;

            assert_eq!(
                balance_at(&pool, id, date("2026-10-31")).await.unwrap(),
                1000
            );
        });
    }

    #[test]
    fn summaries_give_balances_of_accounts_with_operations_only() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let id = checking_account(&pool, 1000).await;
            add_operation(&pool, id, "2026-10-02", -500).await;
            add_operation(&pool, id, "2026-11-02", -200).await;
            account::create(
                &pool,
                NewAccount {
                    is_joint: false,
                    name: "PEA".to_owned(),
                    account_type: AccountType::Investment,
                    currency: "EUR".to_owned(),
                    institution: None,
                    opening_balance_cents: 0,
                    opening_date: Some(date("2026-01-01")),
                },
            )
            .await
            .unwrap();

            let summaries = list_summaries(&pool, false, date("2026-10-31"))
                .await
                .unwrap();
            let balances: Vec<(&str, Option<i64>)> = summaries
                .iter()
                .map(|s| (s.account.name.as_str(), s.balance_cents))
                .collect();
            assert_eq!(balances, [("Compte courant", Some(500)), ("PEA", None)]);
        });
    }

    #[test]
    fn reports_unknown_accounts() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            assert!(matches!(
                balance_at(&pool, 7, date("2026-10-19")).await,
                Err(AppError::Account(AccountError::NotFound(7)))
            ));
        });
    }
}
