use super::*;
use crate::account::{AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn schedule(frequency: Frequency, day: i64, start: &str, end: Option<&str>) -> ScheduledOperation {
    ScheduledOperation {
        id: 1,
        account_id: 1,
        label: "Loyer".to_owned(),
        amount_cents: -112_000,
        category_id: None,
        frequency,
        day_of_period: day,
        start_date: date(start),
        end_date: end.map(date),
    }
}

fn dates(texts: &[&str]) -> Vec<NaiveDate> {
    texts.iter().map(|t| date(t)).collect()
}

#[test]
fn monthly_occurrences_use_the_last_day_of_short_months() {
    let rent = schedule(Frequency::Monthly, 31, "2026-01-01", None);
    assert_eq!(
        occurrences(&rent, date("2026-01-15"), date("2026-05-10")),
        dates(&["2026-01-31", "2026-02-28", "2026-03-31", "2026-04-30"])
    );
    let salary = schedule(Frequency::Monthly, 5, "2026-03-10", Some("2026-06-30"));
    // Not before the start date, not after the end date.
    assert_eq!(
        occurrences(&salary, date("2026-01-01"), date("2026-12-31")),
        dates(&["2026-04-05", "2026-05-05", "2026-06-05"])
    );
}

#[test]
fn quarterly_semiannual_and_yearly_repeat_from_the_start_month() {
    let quarterly = schedule(Frequency::Quarterly, 15, "2026-02-01", None);
    assert_eq!(
        occurrences(&quarterly, date("2026-01-01"), date("2026-12-31")),
        dates(&["2026-02-15", "2026-05-15", "2026-08-15", "2026-11-15"])
    );
    let semiannual = schedule(Frequency::Semiannual, 1, "2025-07-01", None);
    assert_eq!(
        occurrences(&semiannual, date("2026-01-01"), date("2026-12-31")),
        dates(&["2026-01-01", "2026-07-01"])
    );
    let yearly = schedule(Frequency::Yearly, 29, "2024-02-10", None);
    assert_eq!(
        occurrences(&yearly, date("2024-01-01"), date("2026-12-31")),
        dates(&["2024-02-29", "2025-02-28", "2026-02-28"])
    );
}

#[test]
fn weekly_occurrences_fall_on_the_weekday() {
    // 2026-10-20 is a Tuesday; day 5 is Friday.
    let weekly = schedule(Frequency::Weekly, 5, "2026-01-01", None);
    assert_eq!(
        occurrences(&weekly, date("2026-10-20"), date("2026-11-06")),
        dates(&["2026-10-23", "2026-10-30", "2026-11-06"])
    );
    let tuesday = schedule(Frequency::Weekly, 2, "2026-01-01", None);
    assert_eq!(
        occurrences(&tuesday, date("2026-10-20"), date("2026-10-26")),
        dates(&["2026-10-20"])
    );
}

#[test]
fn no_occurrence_outside_the_schedule() {
    let ended = schedule(Frequency::Monthly, 5, "2025-01-01", Some("2025-12-31"));
    assert!(occurrences(&ended, date("2026-01-01"), date("2026-12-31")).is_empty());
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async { test(memory_pool().await).await });
}

async fn new_account(pool: &SqlitePool, account_type: AccountType) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: format!("{account_type:?}"),
            account_type,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: date("2026-01-01"),
        },
    )
    .await
    .unwrap()
    .id
}

fn input(account_id: i64, label: &str, frequency: Frequency, day: i64) -> ScheduledOperationInput {
    ScheduledOperationInput {
        account_id,
        label: label.to_owned(),
        amount_cents: -1_199,
        category_id: None,
        frequency,
        day_of_period: day,
        start_date: date("2026-01-01"),
        end_date: None,
    }
}

#[test]
fn stores_schedules_and_lists_upcoming_occurrences() {
    run(|pool| async move {
        let account = new_account(&pool, AccountType::Checking).await;
        let rent = create(&pool, input(account, "  Loyer ", Frequency::Monthly, 5))
            .await
            .unwrap();
        assert_eq!(rent.label, "Loyer");
        let spotify = create(&pool, input(account, "Spotify", Frequency::Monthly, 25))
            .await
            .unwrap();
        create(&pool, input(account, "Assurance", Frequency::Yearly, 1))
            .await
            .unwrap();

        let upcoming = upcoming(&pool, date("2026-10-20"), 20).await.unwrap();
        let found: Vec<(&str, NaiveDate)> = upcoming
            .iter()
            .map(|u| (u.label.as_str(), u.date))
            .collect();
        assert_eq!(
            found,
            vec![
                ("Spotify", date("2026-10-25")),
                ("Loyer", date("2026-11-05"))
            ]
        );

        let changed = update(
            &pool,
            spotify.id,
            input(account, "Spotify", Frequency::Weekly, 1),
        )
        .await
        .unwrap();
        assert_eq!(changed.frequency, Frequency::Weekly);
        delete(&pool, rent.id).await.unwrap();
        let labels: Vec<String> = list(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.label)
            .collect();
        assert_eq!(labels, ["Assurance", "Spotify"]);
    });
}

#[test]
fn rejects_invalid_schedules() {
    run(|pool| async move {
        let account = new_account(&pool, AccountType::Checking).await;
        let pea = new_account(&pool, AccountType::Investment).await;
        let error = |result: AppResult<ScheduledOperation>| match result {
            Err(AppError::Schedule(error)) => error,
            other => panic!("expected a schedule error, got {other:?}"),
        };
        let base = || input(account, "Loyer", Frequency::Monthly, 5);

        assert!(matches!(
            error(
                create(
                    &pool,
                    ScheduledOperationInput {
                        label: " ".into(),
                        ..base()
                    }
                )
                .await
            ),
            ScheduleError::EmptyLabel
        ));
        assert!(matches!(
            error(
                create(
                    &pool,
                    ScheduledOperationInput {
                        amount_cents: 0,
                        ..base()
                    }
                )
                .await
            ),
            ScheduleError::ZeroAmount
        ));
        assert!(matches!(
            error(
                create(
                    &pool,
                    ScheduledOperationInput {
                        day_of_period: 32,
                        ..base()
                    }
                )
                .await
            ),
            ScheduleError::InvalidDay
        ));
        assert!(matches!(
            error(create(&pool, input(account, "X", Frequency::Weekly, 8)).await),
            ScheduleError::InvalidDay
        ));
        assert!(matches!(
            error(
                create(
                    &pool,
                    ScheduledOperationInput {
                        end_date: Some(date("2025-12-31")),
                        ..base()
                    }
                )
                .await
            ),
            ScheduleError::EndBeforeStart
        ));
        assert!(matches!(
            error(create(&pool, input(pea, "X", Frequency::Monthly, 5)).await),
            ScheduleError::AccountWithoutOperations
        ));
        assert!(matches!(
            error(
                create(
                    &pool,
                    ScheduledOperationInput {
                        category_id: Some(999),
                        ..base()
                    }
                )
                .await
            ),
            ScheduleError::CategoryNotFound(999)
        ));
        assert!(matches!(
            error(update(&pool, 999, base()).await),
            ScheduleError::NotFound(999)
        ));
    });
}
