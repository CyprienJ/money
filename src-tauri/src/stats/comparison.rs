//! Comparison of the expenses by category of two periods (a period versus the previous one,
//! or versus the same period a year earlier).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::{CategoryLevel, expenses_by_category};
use crate::error::AppResult;

/// First and last days of a period, both included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub struct DateRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryComparison {
    /// `None` for uncategorized expenses.
    pub category_id: Option<i64>,
    /// Net expenses of the period.
    pub current_cents: i64,
    /// Net expenses of the compared period.
    pub previous_cents: i64,
}

/// Net expenses by category of `current` and `previous`, for every category with expenses in
/// either period, the largest current expense first.
pub async fn compare_expenses_by_category(
    pool: &SqlitePool,
    current: DateRange,
    previous: DateRange,
    accounts: Option<&[i64]>,
    level: CategoryLevel,
) -> AppResult<Vec<CategoryComparison>> {
    let now = expenses_by_category(pool, current.from, current.to, accounts, level).await?;
    let before = expenses_by_category(pool, previous.from, previous.to, accounts, level).await?;

    // Keyed by category; `None` (uncategorized) sorts first in the map, order is set below.
    let mut rows: BTreeMap<Option<i64>, CategoryComparison> = BTreeMap::new();
    for expense in now {
        rows.entry(expense.category_id)
            .or_insert_with(|| empty(expense.category_id))
            .current_cents = expense.expense_cents;
    }
    for expense in before {
        rows.entry(expense.category_id)
            .or_insert_with(|| empty(expense.category_id))
            .previous_cents = expense.expense_cents;
    }
    let mut rows: Vec<CategoryComparison> = rows.into_values().collect();
    rows.sort_by(|a, b| {
        b.current_cents
            .cmp(&a.current_cents)
            .then(b.previous_cents.cmp(&a.previous_cents))
    });
    Ok(rows)
}

fn empty(category_id: Option<i64>) -> CategoryComparison {
    CategoryComparison {
        category_id,
        current_cents: 0,
        previous_cents: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{self, AccountType, NewAccount};
    use crate::db::tests::memory_pool;
    use crate::error::AppError;
    use crate::operation::{self, NewOperation, category};
    use crate::stats::StatsError;

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    fn range(from: &str, to: &str) -> DateRange {
        DateRange {
            from: date(from),
            to: date(to),
        }
    }

    #[test]
    fn compares_two_periods_category_by_category() {
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
                    opening_date: date("2025-01-01"),
                },
            )
            .await
            .unwrap()
            .id;
            let categories = category::list(&pool).await.unwrap();
            let find = |name: &str| {
                categories
                    .iter()
                    .find(|c| c.name == name && c.parent_id.is_none())
                    .unwrap()
                    .id
            };
            let (housing, groceries, leisure) =
                (find("Logement"), find("Courses"), find("Loisirs"));
            let enter = |day: &'static str, cents: i64, category_id: Option<i64>| {
                let pool = pool.clone();
                async move {
                    operation::create(
                        &pool,
                        NewOperation {
                            account_id,
                            date: date(day),
                            value_date: None,
                            amount_cents: cents,
                            label: "Op".to_owned(),
                            category_id,
                            note: None,
                            tags: Vec::new(),
                        },
                    )
                    .await
                    .unwrap();
                }
            };
            enter("2026-10-05", -93_800, Some(housing)).await;
            enter("2026-10-12", -41_200, Some(groceries)).await;
            enter("2026-10-20", -4_000, None).await;
            enter("2025-10-05", -92_100, Some(housing)).await;
            enter("2025-10-14", -17_500, Some(leisure)).await;

            let rows = compare_expenses_by_category(
                &pool,
                range("2026-10-01", "2026-10-31"),
                range("2025-10-01", "2025-10-31"),
                None,
                CategoryLevel::TopLevel,
            )
            .await
            .unwrap();
            let found: Vec<(Option<i64>, i64, i64)> = rows
                .iter()
                .map(|r| (r.category_id, r.current_cents, r.previous_cents))
                .collect();
            assert_eq!(
                found,
                vec![
                    (Some(housing), 93_800, 92_100),
                    (Some(groceries), 41_200, 0),
                    (None, 4_000, 0),
                    // Spent only in the compared period.
                    (Some(leisure), 0, 17_500),
                ]
            );

            assert!(matches!(
                compare_expenses_by_category(
                    &pool,
                    range("2026-10-31", "2026-10-01"),
                    range("2025-10-01", "2025-10-31"),
                    None,
                    CategoryLevel::TopLevel,
                )
                .await,
                Err(AppError::Stats(StatsError::InvalidPeriod { .. }))
            ));
        });
    }
}
