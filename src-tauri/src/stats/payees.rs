//! Top payees and largest expenses. Same rules as the other expense
//! statistics: internal transfers and income are left out, refunds come off the expenses.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{accounts_json, check_period};
use crate::account::personal_share;
use crate::currency::Rates;
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PayeeExpense {
    /// `None` for operations without a payee yet, grouped by normalized label.
    pub payee_id: Option<i64>,
    pub payee_name: String,
    /// Net amount spent with this payee, positive.
    pub expense_cents: i64,
    pub operation_count: i64,
    /// Category holding most of the amount; `None` when mostly uncategorized.
    pub category_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LargestExpense {
    pub operation_id: i64,
    pub date: NaiveDate,
    pub normalized_label: String,
    pub payee_name: String,
    /// Amount spent, positive.
    pub expense_cents: i64,
    pub category_id: Option<i64>,
}

/// The `limit` payees with the largest net expenses between `from` and `to` (included).
pub async fn top_payees(
    pool: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<&[i64]>,
    limit: u32,
) -> AppResult<Vec<PayeeExpense>> {
    check_period(from, to)?;
    let accounts = accounts_json(accounts);
    let rows = sqlx::query!(
        r#"SELECT o.payee_id, COALESCE(p.name, o.normalized_label) AS "payee_name!: String",
                  s.category_id, a.currency, a.is_joint AS "is_joint: bool", o.date AS "date!: NaiveDate",
                  -s.amount_cents AS "expense_cents!: i64",
                  o.id AS "operation_id!: i64"
           FROM operation_split s
           JOIN operation o ON o.id = s.operation_id
           JOIN account a ON a.id = o.account_id
           LEFT JOIN payee p ON p.id = o.payee_id
           LEFT JOIN category c ON c.id = s.category_id
           WHERE o.date BETWEEN ?1 AND ?2
             AND o.is_internal_transfer = 0
             AND CASE WHEN ?3 IS NULL
                      THEN a.archived = 0
                      ELSE o.account_id IN (SELECT value FROM json_each(?3)) END
             AND NOT COALESCE(c.is_income, s.amount_cents > 0)"#,
        from,
        to,
        accounts
    )
    .fetch_all(pool)
    .await?;

    // Totals in euros by (payee id, payee name, category), and operations by payee: the parts
    // of a split operation count once.
    let rates = Rates::load(pool).await?;
    type Key = (Option<i64>, String, Option<i64>);
    let mut by_category: HashMap<Key, i64> = HashMap::new();
    let mut operations: HashMap<String, HashSet<i64>> = HashMap::new();
    for row in rows {
        operations
            .entry(row.payee_name.clone())
            .or_default()
            .insert(row.operation_id);
        *by_category
            .entry((row.payee_id, row.payee_name, row.category_id))
            .or_default() += personal_share(
            rates.to_reference(row.expense_cents, &row.currency, row.date),
            row.is_joint,
        );
    }

    // Totals by payee, keeping the category with the largest share.
    let mut payees: HashMap<String, (PayeeExpense, i64)> = HashMap::new();
    for ((payee_id, payee_name, category_id), expense_cents) in by_category {
        let (payee, best) = payees.entry(payee_name.clone()).or_insert_with(|| {
            let operation_count = operations.get(&payee_name).map_or(0, HashSet::len);
            let payee = PayeeExpense {
                payee_id,
                payee_name,
                expense_cents: 0,
                operation_count: i64::try_from(operation_count).unwrap_or(i64::MAX),
                category_id: None,
            };
            (payee, i64::MIN)
        });
        payee.expense_cents += expense_cents;
        // Ties go to the smallest category id, whatever the order of the map.
        if (expense_cents, std::cmp::Reverse(category_id))
            > (*best, std::cmp::Reverse(payee.category_id))
        {
            *best = expense_cents;
            payee.category_id = category_id;
        }
    }
    let mut payees: Vec<PayeeExpense> = payees
        .into_values()
        .map(|(payee, _)| payee)
        .filter(|payee| payee.expense_cents > 0)
        .collect();
    payees.sort_by(|a, b| {
        b.expense_cents
            .cmp(&a.expense_cents)
            .then_with(|| a.payee_name.cmp(&b.payee_name))
    });
    payees.truncate(limit as usize);
    Ok(payees)
}

/// The `limit` largest single expenses between `from` and `to` (included).
pub async fn largest_expenses(
    pool: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    accounts: Option<&[i64]>,
    limit: u32,
) -> AppResult<Vec<LargestExpense>> {
    check_period(from, to)?;
    let accounts = accounts_json(accounts);
    let rows = sqlx::query!(
        r#"SELECT o.id AS "operation_id!", o.date AS "date!: NaiveDate", o.normalized_label,
                  COALESCE(p.name, o.normalized_label) AS "payee_name!: String",
                  a.currency, a.is_joint AS "is_joint: bool", -s.amount_cents AS "expense_cents!: i64", s.category_id
           FROM operation_split s
           JOIN operation o ON o.id = s.operation_id
           JOIN account a ON a.id = o.account_id
           LEFT JOIN payee p ON p.id = o.payee_id
           LEFT JOIN category c ON c.id = s.category_id
           WHERE o.date BETWEEN ?1 AND ?2
             AND o.is_internal_transfer = 0
             AND CASE WHEN ?3 IS NULL
                      THEN a.archived = 0
                      ELSE o.account_id IN (SELECT value FROM json_each(?3)) END
             AND s.amount_cents < 0
             AND NOT COALESCE(c.is_income, 0)
           ORDER BY s.id"#,
        from,
        to,
        accounts
    )
    .fetch_all(pool)
    .await?;
    let rates = Rates::load(pool).await?;
    // The expense parts of a split operation make one expense, in the category of its largest part.
    let mut by_operation: HashMap<i64, (LargestExpense, i64)> = HashMap::new();
    for row in rows {
        let cents = personal_share(
            rates.to_reference(row.expense_cents, &row.currency, row.date),
            row.is_joint,
        );
        let (expense, largest_part) = by_operation.entry(row.operation_id).or_insert_with(|| {
            let expense = LargestExpense {
                operation_id: row.operation_id,
                date: row.date,
                normalized_label: row.normalized_label,
                payee_name: row.payee_name,
                expense_cents: 0,
                category_id: row.category_id,
            };
            (expense, i64::MIN)
        });
        expense.expense_cents += cents;
        if cents > *largest_part {
            *largest_part = cents;
            expense.category_id = row.category_id;
        }
    }
    let mut expenses: Vec<LargestExpense> = by_operation
        .into_values()
        .map(|(expense, _)| expense)
        .collect();
    expenses.sort_by_key(|e| (std::cmp::Reverse(e.expense_cents), e.date, e.operation_id));
    expenses.truncate(limit as usize);
    Ok(expenses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{self, AccountType, NewAccount};
    use crate::db::tests::memory_pool;
    use crate::operation::{self, NewOperation, category};
    use crate::payee;

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    #[test]
    fn ranks_payees_and_single_expenses() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let account_id = account::create(
                &pool,
                NewAccount {
                    is_joint: false,
                    name: "Courant".to_owned(),
                    account_type: AccountType::Checking,
                    currency: "EUR".to_owned(),
                    institution: None,
                    opening_balance_cents: 0,
                    opening_date: date("2026-01-01"),
                },
            )
            .await
            .unwrap()
            .id;
            let categories = category::list(&pool).await.unwrap();
            let find = |name: &str| categories.iter().find(|c| c.name == name).unwrap().id;
            let (food, housing, salary, transfer) = (
                find("Alimentation"),
                find("Loyer ou crédit"),
                find("Salaire"),
                find("Virement interne"),
            );
            let enter =
                |day: &'static str, label: &'static str, cents: i64, category_id: Option<i64>| {
                    let pool = pool.clone();
                    async move {
                        operation::create(
                            &pool,
                            NewOperation {
                                account_id,
                                date: date(day),
                                value_date: None,
                                amount_cents: cents,
                                label: label.to_owned(),
                                category_id,
                                splits: Vec::new(),
                                note: None,
                                tags: Vec::new(),
                            },
                        )
                        .await
                        .unwrap()
                        .id
                    }
                };
            enter("2026-09-03", "CB CARREFOUR 02/09", -6_000, Some(food)).await;
            enter("2026-10-03", "CB CARREFOUR 02/10", -9_715, Some(food)).await;
            // A refund comes off the payee total; an uncategorized one too.
            enter("2026-10-04", "CB CARREFOUR 03/10", 715, None).await;
            let loan = enter("2026-10-05", "PRLV CREDIT IMMO", -84_200, Some(housing)).await;
            enter("2026-10-06", "CB SNCF", -8_900, None).await;
            enter("2026-10-01", "VIR SALAIRE", 328_000, Some(salary)).await;
            enter("2026-10-15", "VIR VERS LIVRET A", -30_000, Some(transfer)).await;

            let top = top_payees(&pool, date("2026-01-01"), date("2026-12-31"), None, 5)
                .await
                .unwrap();
            let found: Vec<(&str, i64, i64, Option<i64>)> = top
                .iter()
                .map(|p| {
                    (
                        p.payee_name.as_str(),
                        p.expense_cents,
                        p.operation_count,
                        p.category_id,
                    )
                })
                .collect();
            assert_eq!(
                found,
                vec![
                    // The loan repayment counts, as on the dashboard.
                    ("Credit Immo", 84_200, 1, Some(housing)),
                    // Category of the largest share; the refund is a credit, hence income.
                    ("Carrefour", 15_715, 2, Some(food)),
                    ("Sncf", 8_900, 1, None),
                ]
            );
            // Merged payees count as one.
            let id_of = |name: &str| {
                top.iter()
                    .find(|p| p.payee_name == name)
                    .unwrap()
                    .payee_id
                    .unwrap()
            };
            payee::merge(&pool, id_of("Carrefour"), &[id_of("Sncf")])
                .await
                .unwrap();
            let merged = top_payees(&pool, date("2026-01-01"), date("2026-12-31"), None, 5)
                .await
                .unwrap();
            assert_eq!(merged.len(), 2);
            assert_eq!(
                (merged[1].payee_name.as_str(), merged[1].expense_cents),
                ("Carrefour", 24_615)
            );
            assert_eq!(
                (merged[1].operation_count, merged[1].category_id),
                (3, Some(food))
            );

            let limited = top_payees(&pool, date("2026-01-01"), date("2026-12-31"), None, 1)
                .await
                .unwrap();
            assert_eq!(limited.len(), 1);

            let largest = largest_expenses(&pool, date("2026-10-01"), date("2026-10-31"), None, 2)
                .await
                .unwrap();
            assert_eq!(
                largest,
                vec![
                    LargestExpense {
                        operation_id: loan,
                        date: date("2026-10-05"),
                        normalized_label: "CREDIT IMMO".to_owned(),
                        payee_name: "Credit Immo".to_owned(),
                        expense_cents: 84_200,
                        category_id: Some(housing),
                    },
                    LargestExpense {
                        operation_id: largest[1].operation_id,
                        date: date("2026-10-03"),
                        normalized_label: "CARREFOUR".to_owned(),
                        payee_name: "Carrefour".to_owned(),
                        expense_cents: 9_715,
                        category_id: Some(food),
                    },
                ]
            );
        });
    }

    #[test]
    fn counts_a_split_operation_once() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let account_id = account::create(
                &pool,
                NewAccount {
                    is_joint: false,
                    name: "Courant".to_owned(),
                    account_type: AccountType::Checking,
                    currency: "EUR".to_owned(),
                    institution: None,
                    opening_balance_cents: 0,
                    opening_date: date("2026-01-01"),
                },
            )
            .await
            .unwrap()
            .id;
            let categories = category::list(&pool).await.unwrap();
            let find = |name: &str| categories.iter().find(|c| c.name == name).unwrap().id;
            let (food, housing) = (find("Alimentation"), find("Loyer ou crédit"));
            let id = operation::create(
                &pool,
                NewOperation {
                    account_id,
                    date: date("2026-10-03"),
                    value_date: None,
                    amount_cents: -10_000,
                    label: "CB HYPERMARCHE".to_owned(),
                    category_id: None,
                    splits: vec![
                        operation::SplitLine {
                            category_id: housing,
                            amount_cents: -3_000,
                        },
                        operation::SplitLine {
                            category_id: food,
                            amount_cents: -7_000,
                        },
                    ],
                    note: None,
                    tags: Vec::new(),
                },
            )
            .await
            .unwrap()
            .id;
            let (from, to) = (date("2026-01-01"), date("2026-12-31"));

            let top = top_payees(&pool, from, to, None, 5).await.unwrap();
            assert_eq!(top.len(), 1);
            assert_eq!(
                (top[0].expense_cents, top[0].operation_count, top[0].category_id),
                (10_000, 1, Some(food))
            );
            let largest = largest_expenses(&pool, from, to, None, 5).await.unwrap();
            assert_eq!(largest.len(), 1);
            assert_eq!(
                (largest[0].operation_id, largest[0].expense_cents, largest[0].category_id),
                (id, 10_000, Some(food))
            );
        });
    }
}
