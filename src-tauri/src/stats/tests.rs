use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::{self, NewOperation, category};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

struct Fixture {
    pool: SqlitePool,
    checking: i64,
    savings: i64,
    archived: i64,
    groceries: i64,
    food: i64,
    restaurants: i64,
    restaurants_child: i64,
    leisure: i64,
    sport: i64,
}

async fn new_account(
    pool: &SqlitePool,
    name: &str,
    account_type: AccountType,
    opening: i64,
) -> i64 {
    account::create(
        pool,
        NewAccount {
            name: name.to_owned(),
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

async fn enter(
    pool: &SqlitePool,
    account_id: i64,
    day: &str,
    amount_cents: i64,
    category_id: Option<i64>,
) {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents,
            label: format!("Op {day} {amount_cents}"),
            category_id,
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap();
}

/// Checking (1 000 €), savings (500 €) and credit card accounts, a PEA without operations and
/// an archived account, over January and February 2026.
async fn fixture() -> Fixture {
    let pool = memory_pool().await;
    let categories = category::list(&pool).await.unwrap();
    let find = |name: &str, top_level: bool| {
        categories
            .iter()
            .find(|c| c.name == name && c.parent_id.is_none() == top_level)
            .unwrap()
            .id
    };
    let salary = find("Salaire", false);
    let transfer = find("Virement interne", true);
    let fixture = Fixture {
        checking: new_account(&pool, "Courant", AccountType::Checking, 100_000).await,
        savings: new_account(&pool, "Livret", AccountType::Savings, 50_000).await,
        archived: new_account(&pool, "Ancien", AccountType::Checking, 0).await,
        groceries: find("Courses", true),
        food: find("Alimentation", false),
        restaurants: find("Restaurants", true),
        restaurants_child: find("Restaurants", false),
        leisure: find("Loisirs", true),
        sport: find("Sport", false),
        pool,
    };
    let pool = &fixture.pool;
    let card = new_account(pool, "Carte", AccountType::CreditCard, 0).await;
    new_account(pool, "PEA", AccountType::Investment, 0).await;

    enter(pool, fixture.checking, "2026-01-05", 200_000, Some(salary)).await;
    enter(
        pool,
        fixture.checking,
        "2026-01-10",
        -5_000,
        Some(fixture.food),
    )
    .await;
    // A refund reduces the groceries expenses.
    enter(
        pool,
        fixture.checking,
        "2026-01-12",
        2_000,
        Some(fixture.food),
    )
    .await;
    enter(
        pool,
        fixture.checking,
        "2026-01-15",
        -30_000,
        Some(transfer),
    )
    .await;
    enter(pool, fixture.savings, "2026-01-15", 30_000, Some(transfer)).await;
    enter(pool, fixture.checking, "2026-02-03", -4_000, None).await;
    enter(pool, fixture.checking, "2026-02-04", 1_500, None).await;
    enter(
        pool,
        fixture.checking,
        "2026-02-10",
        -3_000,
        Some(fixture.restaurants_child),
    )
    .await;
    enter(pool, card, "2026-02-20", -10_000, Some(fixture.sport)).await;
    enter(
        pool,
        fixture.archived,
        "2026-01-20",
        -99_900,
        Some(fixture.food),
    )
    .await;
    account::set_archived(pool, fixture.archived, true)
        .await
        .unwrap();
    fixture
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(Fixture) -> F) {
    tauri::async_runtime::block_on(async { test(fixture().await).await });
}

fn expenses(rows: &[CategoryExpense]) -> Vec<(Option<i64>, i64)> {
    rows.iter()
        .map(|r| (r.category_id, r.expense_cents))
        .collect()
}

#[test]
fn expenses_by_top_level_category_exclude_transfers_and_income() {
    run(|f| async move {
        let rows = expenses_by_category(
            &f.pool,
            date("2026-01-01"),
            date("2026-02-28"),
            None,
            CategoryLevel::TopLevel,
        )
        .await
        .unwrap();
        assert_eq!(
            expenses(&rows),
            vec![
                (Some(f.leisure), 10_000),
                // Uncategorized debits only: the uncategorized credit is income.
                (None, 4_000),
                (Some(f.groceries), 3_000),
                (Some(f.restaurants), 3_000),
            ]
        );
    });
}

#[test]
fn expenses_by_detailed_category() {
    run(|f| async move {
        let rows = expenses_by_category(
            &f.pool,
            date("2026-01-01"),
            date("2026-02-28"),
            None,
            CategoryLevel::Detailed,
        )
        .await
        .unwrap();
        assert_eq!(
            expenses(&rows),
            vec![
                (Some(f.sport), 10_000),
                (None, 4_000),
                (Some(f.food), 3_000),
                (Some(f.restaurants_child), 3_000),
            ]
        );
    });
}

#[test]
fn expenses_can_target_accounts_and_dates() {
    run(|f| async move {
        // Explicit accounts include an archived one.
        let rows = expenses_by_category(
            &f.pool,
            date("2026-01-01"),
            date("2026-01-31"),
            Some(&[f.checking, f.archived]),
            CategoryLevel::TopLevel,
        )
        .await
        .unwrap();
        assert_eq!(expenses(&rows), vec![(Some(f.groceries), 3_000 + 99_900)]);

        let rows = expenses_by_category(
            &f.pool,
            date("2026-01-11"),
            date("2026-01-12"),
            None,
            CategoryLevel::TopLevel,
        )
        .await
        .unwrap();
        // Only the refund: negative expenses.
        assert_eq!(expenses(&rows), vec![(Some(f.groceries), -2_000)]);
    });
}

#[test]
fn income_vs_expenses_by_month_and_year() {
    run(|f| async move {
        let months = income_vs_expenses(
            &f.pool,
            date("2026-01-15"),
            date("2026-03-10"),
            None,
            Granularity::Month,
        )
        .await
        .unwrap();
        let found: Vec<(NaiveDate, i64, i64)> = months
            .iter()
            .map(|m| (m.period_start, m.income_cents, m.expense_cents))
            .collect();
        assert_eq!(
            found,
            vec![
                // From the 15th: the salary and groceries are before the period.
                (date("2026-01-01"), 0, 0),
                (date("2026-02-01"), 1_500, 4_000 + 3_000 + 10_000),
                (date("2026-03-01"), 0, 0),
            ]
        );

        let years = income_vs_expenses(
            &f.pool,
            date("2026-01-01"),
            date("2026-12-31"),
            None,
            Granularity::Year,
        )
        .await
        .unwrap();
        assert_eq!(
            years,
            vec![PeriodIncomeExpenses {
                period_start: date("2026-01-01"),
                income_cents: 200_000 + 1_500,
                expense_cents: 3_000 + 4_000 + 3_000 + 10_000,
            }]
        );
        // Income − expenses = net change of the accounts without internal transfers.
        let year = &years[0];
        assert_eq!(year.income_cents - year.expense_cents, 181_500);
    });
}

#[test]
fn balance_history_includes_transfers_and_openings() {
    run(|f| async move {
        let points = balance_history(
            &f.pool,
            date("2026-01-01"),
            date("2026-03-15"),
            None,
            BalanceStep::Month,
        )
        .await
        .unwrap();
        let found: Vec<(NaiveDate, i64)> =
            points.iter().map(|p| (p.date, p.balance_cents)).collect();
        let january = 150_000 + 200_000 - 5_000 + 2_000;
        let february = january - 4_000 + 1_500 - 3_000 - 10_000;
        assert_eq!(
            found,
            vec![
                (date("2026-01-31"), january),
                (date("2026-02-28"), february),
                (date("2026-03-15"), february),
            ]
        );

        let savings = balance_history(
            &f.pool,
            date("2026-01-01"),
            date("2026-01-31"),
            Some(&[f.savings]),
            BalanceStep::Month,
        )
        .await
        .unwrap();
        assert_eq!(savings[0].balance_cents, 80_000);
    });
}

#[test]
fn balance_history_steps() {
    run(|f| async move {
        let days = balance_history(
            &f.pool,
            date("2025-12-31"),
            date("2026-01-02"),
            None,
            BalanceStep::Day,
        )
        .await
        .unwrap();
        let found: Vec<(NaiveDate, i64)> = days.iter().map(|p| (p.date, p.balance_cents)).collect();
        // Nothing before the accounts open.
        assert_eq!(
            found,
            vec![
                (date("2025-12-31"), 0),
                (date("2026-01-01"), 150_000),
                (date("2026-01-02"), 150_000),
            ]
        );

        let weeks = balance_history(
            &f.pool,
            date("2026-01-01"),
            date("2026-01-20"),
            None,
            BalanceStep::Week,
        )
        .await
        .unwrap();
        let dates: Vec<NaiveDate> = weeks.iter().map(|p| p.date).collect();
        assert_eq!(
            dates,
            vec![
                date("2026-01-01"),
                date("2026-01-08"),
                date("2026-01-15"),
                date("2026-01-20")
            ]
        );
    });
}

#[test]
fn rejects_reversed_periods() {
    run(|f| async move {
        let from = date("2026-02-01");
        let to = date("2026-01-01");
        assert!(matches!(
            expenses_by_category(&f.pool, from, to, None, CategoryLevel::TopLevel).await,
            Err(AppError::Stats(StatsError::InvalidPeriod { .. }))
        ));
        assert!(matches!(
            income_vs_expenses(&f.pool, from, to, None, Granularity::Month).await,
            Err(AppError::Stats(StatsError::InvalidPeriod { .. }))
        ));
        assert!(matches!(
            balance_history(&f.pool, from, to, None, BalanceStep::Day).await,
            Err(AppError::Stats(StatsError::InvalidPeriod { .. }))
        ));
    });
}
