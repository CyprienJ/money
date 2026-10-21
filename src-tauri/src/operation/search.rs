//! Operations matching a combination of filters, over one account or all of them.

use chrono::NaiveDate;
use serde::Deserialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{Operation, OperationRow};
use crate::error::AppResult;
use crate::tag;

#[cfg(test)]
mod tests;

/// Sign of the operations to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// Negative amounts.
    Debit,
    /// Positive amounts.
    Credit,
}

/// Category of the operations to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CategoryFilter {
    /// Operations still to categorize.
    Uncategorized,
    /// Operations of the category, or of one of its subcategories.
    Category { id: i64 },
}

/// Filters combined with AND; a `None` filter keeps everything. Bounds are included.
#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationFilter {
    /// `None` for every account.
    pub account_id: Option<i64>,
    /// Whether archived accounts are searched when `account_id` is `None`.
    pub include_archived: bool,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub direction: Option<Direction>,
    /// Lowest amount, in absolute value.
    pub min_cents: Option<i64>,
    /// Highest amount, in absolute value.
    pub max_cents: Option<i64>,
    pub payee_id: Option<i64>,
    pub category: Option<CategoryFilter>,
    pub tag_id: Option<i64>,
}

/// Operations matching `filter`, most recent first. Each row carries the balance of its account
/// right after the operation.
pub async fn search(pool: &SqlitePool, filter: &OperationFilter) -> AppResult<Vec<OperationRow>> {
    let direction = filter.direction.map(|d| match d {
        Direction::Debit => "debit",
        Direction::Credit => "credit",
    });
    let (uncategorized, category_id) = match filter.category {
        None => (false, None),
        Some(CategoryFilter::Uncategorized) => (true, None),
        Some(CategoryFilter::Category { id }) => (false, Some(id)),
    };

    // Running balances are computed over the whole history of each account, then filtered.
    // V1 has a single split per operation, so the join does not duplicate rows.
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", account_id AS "account_id!", date AS "date!: NaiveDate",
                  value_date AS "value_date: NaiveDate", amount_cents AS "amount_cents!",
                  raw_label AS "raw_label!", normalized_label AS "normalized_label!",
                  payee_id, payee_name AS "payee_name!: String", import_batch_id, category_id,
                  is_internal_transfer AS "is_internal_transfer!: bool", transfer_pair_id, note,
                  (SELECT COUNT(*) FROM attachment WHERE operation_id = id)
                      AS "attachment_count!: i64",
                  balance_cents AS "balance_cents!: i64"
           FROM (
               SELECT o.id, o.account_id, o.date, o.value_date, o.amount_cents, o.raw_label,
                      o.normalized_label, o.payee_id,
                      COALESCE(p.name, o.normalized_label) AS payee_name,
                      o.import_batch_id, s.category_id, o.is_internal_transfer,
                      o.transfer_pair_id, o.note,
                      a.archived,
                      a.opening_balance_cents + SUM(o.amount_cents)
                          OVER (PARTITION BY o.account_id ORDER BY o.date, o.id) AS balance_cents
               FROM operation o
               JOIN account a ON a.id = o.account_id
               LEFT JOIN operation_split s ON s.operation_id = o.id
               LEFT JOIN payee p ON p.id = o.payee_id
               WHERE ?1 IS NULL OR o.account_id = ?1
           )
           WHERE (?1 IS NOT NULL OR ?2 OR NOT archived)
             AND (?3 IS NULL OR date >= ?3) AND (?4 IS NULL OR date <= ?4)
             AND (?5 IS NULL OR (?5 = 'debit' AND amount_cents < 0)
                             OR (?5 = 'credit' AND amount_cents > 0))
             AND (?6 IS NULL OR ABS(amount_cents) >= ?6)
             AND (?7 IS NULL OR ABS(amount_cents) <= ?7)
             AND (?8 IS NULL OR payee_id = ?8)
             AND (NOT ?9 OR category_id IS NULL)
             AND (?10 IS NULL OR category_id IN
                     (SELECT c.id FROM category c WHERE c.id = ?10 OR c.parent_id = ?10))
             AND (?11 IS NULL OR EXISTS
                     (SELECT 1 FROM operation_tag ot WHERE ot.operation_id = id AND ot.tag_id = ?11))
           ORDER BY date DESC, id DESC"#,
        filter.account_id,
        filter.include_archived,
        filter.from,
        filter.to,
        direction,
        filter.min_cents,
        filter.max_cents,
        filter.payee_id,
        uncategorized,
        category_id,
        filter.tag_id
    )
    .fetch_all(pool)
    .await?;

    let mut tags = tag::names_by_operation(pool, filter.account_id).await?;
    Ok(rows
        .into_iter()
        .map(|row| OperationRow {
            operation: Operation {
                id: row.id,
                account_id: row.account_id,
                date: row.date,
                value_date: row.value_date,
                amount_cents: row.amount_cents,
                raw_label: row.raw_label,
                normalized_label: row.normalized_label,
                payee_id: row.payee_id,
                payee_name: row.payee_name,
                import_batch_id: row.import_batch_id,
                category_id: row.category_id,
                is_internal_transfer: row.is_internal_transfer,
                transfer_pair_id: row.transfer_pair_id,
                note: row.note,
                tags: tags.remove(&row.id).unwrap_or_default(),
                attachment_count: row.attachment_count,
            },
            balance_cents: row.balance_cents,
        })
        .collect())
}

/// Operations of an account, most recent first, optionally restricted to a period. A shortcut
/// for tests.
#[cfg(test)]
pub async fn list(
    pool: &SqlitePool,
    account_id: i64,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> AppResult<Vec<OperationRow>> {
    let filter = OperationFilter {
        account_id: Some(account_id),
        from,
        to,
        ..OperationFilter::default()
    };
    search(pool, &filter).await
}
