//! Top payees and largest expenses. Same rules as the other expense
//! statistics: internal transfers and income are left out, refunds come off the expenses.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{accounts_json, check_period};
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
                  s.category_id,
                  -SUM(s.amount_cents) AS "expense_cents!: i64",
                  COUNT(*) AS "operation_count!: i64"
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
             AND NOT COALESCE(c.is_income, s.amount_cents > 0)
           GROUP BY o.payee_id, 2, s.category_id"#,
        from,
        to,
        accounts
    )
    .fetch_all(pool)
    .await?;

    // Totals by payee, keeping the category with the largest share.
    let mut payees: HashMap<String, (PayeeExpense, i64)> = HashMap::new();
    for row in rows {
        let (payee, best) = payees.entry(row.payee_name.clone()).or_insert_with(|| {
            let payee = PayeeExpense {
                payee_id: row.payee_id,
                payee_name: row.payee_name.clone(),
                expense_cents: 0,
                operation_count: 0,
                category_id: None,
            };
            (payee, i64::MIN)
        });
        payee.expense_cents += row.expense_cents;
        payee.operation_count += row.operation_count;
        if row.expense_cents > *best {
            *best = row.expense_cents;
            payee.category_id = row.category_id;
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
    let expenses = sqlx::query_as!(
        LargestExpense,
        r#"SELECT o.id AS "operation_id!", o.date AS "date!: NaiveDate", o.normalized_label,
                  COALESCE(p.name, o.normalized_label) AS "payee_name!: String",
                  -s.amount_cents AS "expense_cents!: i64", s.category_id
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
           ORDER BY s.amount_cents, o.date, o.id
           LIMIT ?4"#,
        from,
        to,
        accounts,
        limit
    )
    .fetch_all(pool)
    .await?;
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
                    name: "Courant".to_owned(),
                    account_type: AccountType::Checking,
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
}
