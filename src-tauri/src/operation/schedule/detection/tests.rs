use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::schedule::{self, ScheduledOperationInput, matching};
use crate::operation::{self, NewOperation};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn op(day: &str, amount_cents: i64) -> SeriesOperation {
    SeriesOperation {
        date: date(day),
        amount_cents,
        category_id: None,
        matched: false,
    }
}

/// Frequency and dates of the series found in `operations`, most recent first.
fn found(operations: &[SeriesOperation]) -> Option<(Frequency, Vec<NaiveDate>)> {
    series(operations).map(|(frequency, chain)| (frequency, chain.iter().map(|o| o.date).collect()))
}

#[test]
fn recognizes_each_frequency() {
    let weekly = [
        op("2026-03-02", -500),
        op("2026-03-09", -500),
        op("2026-03-16", -500),
    ];
    assert_eq!(found(&weekly).unwrap().0, Frequency::Weekly);
    // A shorter month and a payment delayed by a weekend.
    let monthly = [
        op("2026-01-30", -1_299),
        op("2026-02-28", -1_299),
        op("2026-04-01", -1_299),
    ];
    assert_eq!(found(&monthly).unwrap().0, Frequency::Monthly);
    let quarterly = [
        op("2025-10-15", -9_000),
        op("2026-01-15", -9_000),
        op("2026-04-15", -9_000),
    ];
    assert_eq!(found(&quarterly).unwrap().0, Frequency::Quarterly);
    let semiannual = [
        op("2025-03-01", -30_000),
        op("2025-09-01", -30_000),
        op("2026-03-01", -30_000),
    ];
    assert_eq!(found(&semiannual).unwrap().0, Frequency::Semiannual);
    // Two operations are enough for a yearly series.
    let yearly = [op("2025-03-10", -45_000), op("2026-03-12", -46_000)];
    assert_eq!(found(&yearly).unwrap().0, Frequency::Yearly);
}

#[test]
fn needs_enough_regular_operations_with_close_amounts() {
    assert_eq!(
        found(&[op("2026-02-05", -1_299), op("2026-03-05", -1_299)]),
        None
    );
    // An amount 20 % off breaks the series.
    let changed = [
        op("2026-01-05", -1_000),
        op("2026-02-05", -1_299),
        op("2026-03-05", -1_299),
    ];
    assert_eq!(found(&changed), None);
    // A gap of two months breaks it too.
    let gap = [
        op("2025-12-05", -1_299),
        op("2026-01-05", -1_299),
        op("2026-03-05", -1_299),
    ];
    assert_eq!(found(&gap), None);
}

#[test]
fn skips_unrelated_operations_in_between() {
    let operations = [
        op("2026-01-05", -1_299),
        op("2026-01-20", -4_500),
        op("2026-02-05", -1_299),
        op("2026-02-07", -1_350),
        op("2026-03-05", -1_299),
    ];
    assert_eq!(
        found(&operations),
        Some((
            Frequency::Monthly,
            vec![date("2026-03-05"), date("2026-02-05"), date("2026-01-05")]
        ))
    );
}

#[test]
fn picks_the_median_day_and_the_current_weekday() {
    let monthly = [
        op("2026-01-04", -1),
        op("2026-02-05", -1),
        op("2026-03-09", -1),
    ];
    let (frequency, chain) = series(&monthly).unwrap();
    assert_eq!(day_of_period(frequency, &chain), 5);
    // March 16, 2026 is a Monday.
    let weekly = [
        op("2026-03-02", -1),
        op("2026-03-09", -1),
        op("2026-03-16", -1),
    ];
    let (frequency, chain) = series(&weekly).unwrap();
    assert_eq!(day_of_period(frequency, &chain), 1);
}

#[test]
fn ends_a_series_after_a_missed_period() {
    assert!(is_active(
        Frequency::Monthly,
        date("2026-03-05"),
        date("2026-04-14")
    ));
    assert!(!is_active(
        Frequency::Monthly,
        date("2026-03-05"),
        date("2026-04-15")
    ));
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        test(memory_pool().await).await;
    });
}

async fn new_account(pool: &SqlitePool) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: "Courant".to_owned(),
            account_type: AccountType::Checking,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: Some(date("2025-01-01")),
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
    label: &str,
    amount_cents: i64,
) -> i64 {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents,
            label: label.to_owned(),
            category_id: None,
            splits: Vec::new(),
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
    .id
}

/// Netflix every month, a salary every month, and one-off purchases.
async fn sample(pool: &SqlitePool) -> i64 {
    let account_id = new_account(pool).await;
    for day in ["2026-01-12", "2026-02-12", "2026-03-12"] {
        enter(pool, account_id, day, "NETFLIX", -1_399).await;
    }
    for day in ["2026-01-28", "2026-02-27", "2026-03-27"] {
        enter(pool, account_id, day, "SALAIRE ACME", 250_000).await;
    }
    enter(pool, account_id, "2026-02-14", "FNAC", -4_500).await;
    enter(pool, account_id, "2026-03-01", "FNAC", -3_000).await;
    account_id
}

fn labels(suggestions: &[RecurringSuggestion]) -> Vec<(&str, Frequency, i64)> {
    suggestions
        .iter()
        .map(|s| (s.payee_name.as_str(), s.frequency, s.amount_cents))
        .collect()
}

#[test]
fn suggests_uncovered_active_series() {
    run(|pool| async move {
        let account_id = sample(&pool).await;
        let found = suggestions(&pool, date("2026-04-01")).await.unwrap();
        assert_eq!(
            labels(&found),
            [
                ("Netflix", Frequency::Monthly, -1_399),
                ("Salaire Acme", Frequency::Monthly, 250_000)
            ]
        );
        let netflix = &found[0];
        assert_eq!(netflix.account_id, account_id);
        assert_eq!(netflix.day_of_period, 12);
        assert_eq!(netflix.start_date, date("2026-01-12"));
        assert_eq!(netflix.last_date, date("2026-03-12"));
        assert_eq!(netflix.occurrence_count, 3);

        // Three months later, both series are over.
        assert!(
            suggestions(&pool, date("2026-07-01"))
                .await
                .unwrap()
                .is_empty()
        );
    });
}

#[test]
fn leaves_out_series_covered_or_dismissed() {
    run(|pool| async move {
        let account_id = sample(&pool).await;
        let today = date("2026-04-01");
        let netflix = suggestions(&pool, today).await.unwrap()[0].clone();

        // Added to the schedule, with its past occurrences matched.
        let id = schedule::create(
            &pool,
            ScheduledOperationInput {
                account_id,
                label: netflix.payee_name.clone(),
                amount_cents: netflix.amount_cents,
                category_id: None,
                frequency: netflix.frequency,
                day_of_period: netflix.day_of_period,
                start_date: netflix.start_date,
                end_date: None,
            },
        )
        .await
        .unwrap()
        .id;
        assert_eq!(
            matching::match_certain(&pool, today, None).await.unwrap(),
            3
        );
        assert_eq!(
            labels(&suggestions(&pool, today).await.unwrap()),
            [("Salaire Acme", Frequency::Monthly, 250_000)]
        );
        // Without its matches, the schedule still covers the series.
        schedule::delete(&pool, id).await.unwrap();
        assert_eq!(suggestions(&pool, today).await.unwrap().len(), 2);

        let salary = suggestions(&pool, today).await.unwrap()[1].clone();
        dismiss(&pool, account_id, salary.payee_id, true)
            .await
            .unwrap();
        assert_eq!(
            labels(&suggestions(&pool, today).await.unwrap()),
            [("Netflix", Frequency::Monthly, -1_399)]
        );
        assert!(matches!(
            dismiss(&pool, account_id, 999, true).await,
            Err(AppError::Payee(PayeeError::NotFound(999)))
        ));
    });
}

#[test]
fn a_schedule_of_another_frequency_does_not_cover_a_series() {
    run(|pool| async move {
        let account_id = sample(&pool).await;
        let schedules = schedule::list(&pool).await.unwrap();
        assert!(!covered(&schedules, account_id, Frequency::Monthly, -1_399));
        schedule::create(
            &pool,
            ScheduledOperationInput {
                account_id,
                label: "Streaming".to_owned(),
                amount_cents: -1_399,
                category_id: None,
                frequency: Frequency::Yearly,
                day_of_period: 12,
                start_date: date("2026-01-12"),
                end_date: None,
            },
        )
        .await
        .unwrap();
        let schedules = schedule::list(&pool).await.unwrap();
        assert!(!covered(&schedules, account_id, Frequency::Monthly, -1_399));
        assert!(covered(&schedules, account_id, Frequency::Yearly, -1_450));
    });
}
