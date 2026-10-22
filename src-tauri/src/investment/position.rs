use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use super::{QUANTITY_SCALE, TransactionKind};
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
    /// Dividends received since the first buy.
    pub dividends_cents: i64,
}

/// Quantity, cost and dividends of a security, transaction after transaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Holding {
    pub quantity_micro: i64,
    pub cost_cents: i64,
    pub dividends_cents: i64,
}

/// `a × b / c` rounded half up, for non-negative values.
fn mul_div(a: i64, b: i64, c: i64) -> i64 {
    let (a, b, c) = (i128::from(a), i128::from(b), i128::from(c));
    ((a * b * 2 + c) / (c * 2)) as i64
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
                Holding {
                    quantity_micro: remaining,
                    cost_cents: mul_div(self.cost_cents, remaining, self.quantity_micro),
                    ..self
                }
            }
            TransactionKind::Dividend => Holding {
                dividends_cents: self.dividends_cents + amount_cents,
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
}

/// Securities held in an investment account at the end of `date`, by name.
pub async fn positions(
    pool: &SqlitePool,
    account_id: i64,
    date: NaiveDate,
) -> AppResult<Vec<Position>> {
    let rows = sqlx::query!(
        r#"SELECT t.security_id, s.name, s.isin, t.kind AS "kind: TransactionKind",
                  t.quantity_micro, t.amount_cents, t.fees_cents
           FROM investment_transaction t
           JOIN security s ON s.id = t.security_id
           WHERE t.account_id = ?1 AND t.date <= ?2
           ORDER BY t.date, t.id"#,
        account_id,
        date
    )
    .fetch_all(pool)
    .await?;
    let mut holdings: BTreeMap<i64, (String, Option<String>, Holding)> = BTreeMap::new();
    for row in rows {
        let entry = holdings
            .entry(row.security_id)
            .or_insert_with(|| (row.name, row.isin, Holding::default()));
        // The transactions are checked on every change: a sell never exceeds the holding.
        entry.2 = entry
            .2
            .apply(
                row.kind,
                row.quantity_micro,
                row.amount_cents,
                row.fees_cents,
            )
            .unwrap_or_default();
    }
    let mut positions: Vec<Position> = holdings
        .into_iter()
        .filter(|(_, (_, _, holding))| holding.quantity_micro > 0)
        .map(|(security_id, (security_name, isin, holding))| Position {
            security_id,
            security_name,
            isin,
            quantity_micro: holding.quantity_micro,
            cost_cents: holding.cost_cents,
            unit_cost_cents: holding.unit_cost_cents(),
            dividends_cents: holding.dividends_cents,
        })
        .collect();
    positions.sort_by(|a, b| {
        a.security_name
            .to_lowercase()
            .cmp(&b.security_name.to_lowercase())
    });
    Ok(positions)
}
