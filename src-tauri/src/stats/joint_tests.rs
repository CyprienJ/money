use super::*;
use crate::account::valuation::{self, ValuationInput};
use crate::account::{self, AccountType, AccountUpdate, NewAccount};
use crate::currency;
use crate::db::tests::memory_pool;
use crate::operation::{self, NewOperation, category, schedule};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

async fn create(pool: &SqlitePool, joint: bool, kind: AccountType, opening: i64) -> i64 {
    account::create(
        pool,
        NewAccount {
            name: format!("{kind:?} {joint}"),
            account_type: kind,
            is_joint: joint,
            currency: "EUR".into(),
            institution: None,
            opening_balance_cents: opening,
            opening_date: Some(date("2026-01-01")),
        },
    )
    .await
    .unwrap()
    .id
}

async fn enter(pool: &SqlitePool, account_id: i64, amount: i64, category_id: Option<i64>) -> i64 {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date("2026-02-01"),
            value_date: None,
            amount_cents: amount,
            label: "Shop".into(),
            category_id,
            splits: Vec::new(),
            note: None,
            tags: vec![],
        },
    )
    .await
    .unwrap()
    .id
}

async fn joint(pool: &SqlitePool, id: i64, is_joint: bool) {
    let a = account::get(pool, id).await.unwrap();
    let updated = account::update(
        pool,
        id,
        AccountUpdate {
            name: a.name,
            currency: a.currency,
            institution: a.institution,
            opening_balance_cents: a.opening_balance_cents,
            opening_date: a.opening_date,
            is_joint,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.is_joint, is_joint);
}

#[test]
fn personal_flows_keep_bank_amounts_and_apply_to_history_and_filters() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let solo = create(&pool, false, AccountType::Checking, 1000).await;
        let shared = create(&pool, false, AccountType::Checking, 101).await;
        let cats = category::list(&pool).await.unwrap();
        let find = |name: &str| cats.iter().find(|c| c.name == name).unwrap().id;
        let food = find("Alimentation");
        let transfer = find("Virement interne");
        let salary = find("Salaire");
        enter(&pool, solo, -100, Some(food)).await;
        let expense = enter(&pool, shared, -201, Some(food)).await;
        enter(&pool, shared, 3, Some(food)).await;
        enter(&pool, shared, 301, Some(salary)).await;
        enter(&pool, solo, -200, Some(transfer)).await;
        enter(&pool, shared, 200, Some(transfer)).await;
        joint(&pool, shared, true).await;
        let (from, to) = (date("2026-02-01"), date("2026-02-28"));
        let categories = expenses_by_category(&pool, from, to, None, CategoryLevel::TopLevel)
            .await
            .unwrap();
        assert_eq!(categories.iter().map(|c| c.expense_cents).sum::<i64>(), 199);
        let periods = income_vs_expenses(&pool, from, to, None, Granularity::Month)
            .await
            .unwrap();
        assert_eq!(
            (periods[0].expense_cents, periods[0].income_cents),
            (199, 151)
        );
        let filtered = income_vs_expenses(&pool, from, to, Some(&[shared]), Granularity::Month)
            .await
            .unwrap();
        assert_eq!(filtered[0].expense_cents, 99);
        let top = top_payees(&pool, from, to, None, 5).await.unwrap();
        assert_eq!((top[0].expense_cents, top[0].operation_count), (199, 3));
        let largest = largest_expenses(&pool, from, to, None, 1).await.unwrap();
        assert_eq!(
            (largest[0].operation_id, largest[0].expense_cents),
            (expense, 101)
        );
        let comparison = compare_expenses_by_category(
            &pool,
            DateRange { from, to },
            DateRange {
                from: date("2026-01-01"),
                to: date("2026-01-31"),
            },
            None,
            CategoryLevel::TopLevel,
        )
        .await
        .unwrap();
        assert_eq!(comparison[0].current_cents, 199);
        assert_eq!(
            operation::get(&pool, expense).await.unwrap().amount_cents,
            -201
        );
        assert_eq!(account::balance_at(&pool, shared, to).await.unwrap(), 404);
        let summaries = account::list_summaries(&pool, false, to).await.unwrap();
        let summary = summaries.iter().find(|s| s.account.id == shared).unwrap();
        assert_eq!(
            (summary.balance_cents, summary.personal_balance_cents),
            (Some(404), Some(202))
        );
        let history = balance_history(&pool, to, to, None, BalanceStep::Day)
            .await
            .unwrap();
        assert_eq!(history[0].balance_cents, 902);
        assert_eq!(net_worth_at(&pool, to).await.unwrap().cash_cents, 902);
        account::set_archived(&pool, shared, true).await.unwrap();
        assert_eq!(
            income_vs_expenses(&pool, from, to, None, Granularity::Month)
                .await
                .unwrap()[0]
                .expense_cents,
            100
        );
        joint(&pool, shared, false).await;
        assert_eq!(
            income_vs_expenses(&pool, from, to, Some(&[shared]), Granularity::Month)
                .await
                .unwrap()[0]
                .expense_cents,
            198
        );
    });
}

#[test]
fn values_joint_assets_debts_and_portfolios_at_half() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let day = date("2026-02-01");
        for (kind, value) in [
            (AccountType::Investment, 301),
            (AccountType::Asset, 501),
            (AccountType::Liability, 101),
        ] {
            let id = create(&pool, true, kind, 0).await;
            valuation::create(
                &pool,
                id,
                ValuationInput {
                    date: day,
                    value_cents: value,
                },
            )
            .await
            .unwrap();
        }
        let worth = net_worth_at(&pool, day).await.unwrap();
        assert_eq!(
            (
                worth.investments_cents,
                worth.assets_cents,
                worth.liabilities_cents,
                worth.net_worth_cents
            ),
            (151, 251, 51, 351)
        );
        let history = net_worth_history(&pool, day, day, BalanceStep::Day)
            .await
            .unwrap();
        assert_eq!(history[0], worth);
        // A portfolio uses its cost without prices, while the portfolio itself remains whole.
        let id = create(&pool, true, AccountType::Investment, 0).await;
        use crate::investment::{self, SecurityInput, TransactionInput, TransactionKind};
        let security = investment::securities::create(
            &pool,
            SecurityInput {
                name: "Fund".into(),
                isin: None,
                symbol: None,
                currency: "EUR".into(),
            },
        )
        .await
        .unwrap();
        investment::transactions::create(
            &pool,
            id,
            TransactionInput {
                security_id: security.id,
                date: day,
                kind: TransactionKind::Buy,
                quantity_micro: 1_000_000,
                amount_cents: 101,
                fees_cents: 0,
                note: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            net_worth_at(&pool, day).await.unwrap().investments_cents,
            202
        );
    });
}

#[test]
fn forecast_rounds_projected_account_balance_and_preserves_occurrences() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let id = create(&pool, true, AccountType::Checking, 101).await;
        schedule::create(
            &pool,
            schedule::ScheduledOperationInput {
                account_id: id,
                label: "Rent".into(),
                amount_cents: -1,
                category_id: None,
                frequency: schedule::Frequency::Monthly,
                day_of_period: 2,
                start_date: date("2026-02-01"),
                end_date: None,
            },
        )
        .await
        .unwrap();
        let result = balance_forecast(&pool, date("2026-02-01"), date("2026-02-02"), Some(&[id]))
            .await
            .unwrap();
        assert_eq!(result.current_balance_cents, 51);
        assert_eq!(result.points[1].balance_cents, 50);
        assert_eq!(result.occurrences[0].amount_cents, -1);
        assert_eq!(
            balance_forecast(&pool, date("2026-02-01"), date("2026-02-02"), Some(&[]))
                .await
                .unwrap()
                .current_balance_cents,
            0
        );
    });
}

#[test]
fn converts_before_halving_and_rounds_negative_balances() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let id = create(&pool, true, AccountType::Checking, -3).await;
        let a = account::get(&pool, id).await.unwrap();
        account::update(
            &pool,
            id,
            AccountUpdate {
                name: a.name,
                currency: "USD".into(),
                is_joint: true,
                institution: None,
                opening_balance_cents: -3,
                opening_date: a.opening_date,
            },
        )
        .await
        .unwrap();
        currency::set_manual_rate(&pool, "USD", date("2026-01-01"), 800_000_000)
            .await
            .unwrap();
        let day = date("2026-02-01");
        enter(&pool, id, -3, None).await;
        assert_eq!(
            income_vs_expenses(&pool, day, day, None, Granularity::Month)
                .await
                .unwrap()[0]
                .expense_cents,
            1
        );
        assert_eq!(net_worth_at(&pool, day).await.unwrap().cash_cents, -3);
        let summaries = account::list_summaries(&pool, false, day).await.unwrap();
        assert_eq!(summaries[0].personal_balance_cents, Some(-3));
    });
}

#[test]
fn migration_preserves_existing_accounts_as_solo() {
    tauri::async_runtime::block_on(async {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().in_memory(true))
            .await
            .unwrap();
        sqlx::raw_sql("CREATE TABLE account (id INTEGER PRIMARY KEY, name TEXT, opening_balance_cents INTEGER); INSERT INTO account VALUES (1, 'Existing', 123);")
            .execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!("../../migrations/0014_joint_accounts.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let before: (String, i64, bool) = sqlx::query_as(
            "SELECT name, opening_balance_cents, is_joint FROM account WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(before, ("Existing".into(), 123, false));
        assert!(
            sqlx::query("UPDATE account SET is_joint = 2")
                .execute(&pool)
                .await
                .is_err()
        );
    });
}
