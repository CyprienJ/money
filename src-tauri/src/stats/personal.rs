//! Keep raw balances by account until conversion and ownership rounding.
use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::account::personal_share;
use crate::currency::Rates;

#[derive(Default)]
pub(super) struct PersonalBalances(BTreeMap<i64, (String, bool, i64)>);

impl PersonalBalances {
    pub fn add(&mut self, id: i64, currency: &str, is_joint: bool, cents: i64) {
        self.0
            .entry(id)
            .or_insert_with(|| (currency.to_owned(), is_joint, 0))
            .2 += cents;
    }

    pub fn add_all(&mut self, other: &Self) {
        for (&id, (currency, is_joint, cents)) in &other.0 {
            self.add(id, currency, *is_joint, *cents);
        }
    }

    pub fn to_reference(&self, rates: &Rates, date: NaiveDate) -> i64 {
        self.0
            .values()
            .map(|(currency, joint, cents)| {
                personal_share(rates.to_reference(*cents, currency, date), *joint)
            })
            .sum()
    }
}
