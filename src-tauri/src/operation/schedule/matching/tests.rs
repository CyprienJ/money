use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::schedule::{self, Frequency, ScheduledOperationInput, occurrences_between};
use crate::operation::{self, NewOperation};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        test(memory_pool().await).await;
    });
}

async fn new_account(pool: &SqlitePool, name: &str) -> i64 {
    account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: name.to_owned(),
            account_type: AccountType::Checking,
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

fn rent_input(account_id: i64, day: i64) -> ScheduledOperationInput {
    ScheduledOperationInput {
        account_id,
        label: "Loyer".to_owned(),
        amount_cents: -80_000,
        category_id: None,
        frequency: Frequency::Monthly,
        day_of_period: day,
        start_date: date("2026-01-05"),
        end_date: None,
    }
}

/// Monthly rent of 800 € on the 5th, from January 2026.
async fn rent(pool: &SqlitePool, account_id: i64) -> i64 {
    schedule::create(pool, rent_input(account_id, 5))
        .await
        .unwrap()
        .id
}

async fn enter(pool: &SqlitePool, account_id: i64, day: &str, amount_cents: i64) -> i64 {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents,
            label: format!("PRLV {amount_cents}"),
            category_id: None,
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
    .id
}

/// (occurrence date, operation id) of the realized occurrences between `from` and `to`.
async fn realized(pool: &SqlitePool, from: &str, to: &str) -> Vec<(NaiveDate, i64)> {
    occurrences_between(pool, date(from), date(to))
        .await
        .unwrap()
        .into_iter()
        .filter_map(|o| o.realized_by.map(|r| (o.date, r.operation_id)))
        .collect()
}

async fn proposed(pool: &SqlitePool, today: &str) -> Vec<(NaiveDate, i64)> {
    proposals(pool, date(today))
        .await
        .unwrap()
        .into_iter()
        .map(|p| (p.occurrence.date, p.operation.operation_id))
        .collect()
}

#[test]
fn checks_the_amount_tolerance_and_direction() {
    assert!(amount_matches(-80_000, -80_000));
    assert!(amount_matches(-80_000, -88_000));
    assert!(amount_matches(-80_000, -72_000));
    assert!(!amount_matches(-80_000, -88_001));
    assert!(!amount_matches(-80_000, 80_000));
}

#[test]
fn realizes_occurrences_with_certain_matches() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let other = new_account(&pool, "Autre").await;
        rent(&pool, checking).await;
        let march = enter(&pool, checking, "2026-03-06", -82_000).await;
        // Too far, too different, the other direction, another account.
        enter(&pool, checking, "2026-02-11", -80_000).await;
        enter(&pool, checking, "2026-01-05", -90_000).await;
        enter(&pool, checking, "2026-04-05", 80_000).await;
        enter(&pool, other, "2026-04-05", -80_000).await;

        // Not in this import: nothing matched.
        assert_eq!(
            match_certain(&pool, date("2026-04-20"), Some(999))
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            match_certain(&pool, date("2026-04-20"), None)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            realized(&pool, "2026-01-01", "2026-04-30").await,
            [(date("2026-03-05"), march)]
        );
        assert!(proposed(&pool, "2026-04-20").await.is_empty());
    });
}

#[test]
fn matches_an_operation_paid_before_its_occurrence() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        rent(&pool, checking).await;
        let early = enter(&pool, checking, "2026-04-02", -80_000).await;
        // The occurrence of April 5 is within the detection window of April 1.
        assert_eq!(
            match_certain(&pool, date("2026-04-01"), None)
                .await
                .unwrap(),
            1
        );
        let upcoming = schedule::upcoming(&pool, date("2026-04-03"), 30)
            .await
            .unwrap();
        assert_eq!(
            upcoming[0].realized_by.as_ref().map(|r| r.operation_id),
            Some(early)
        );
    });
}

#[test]
fn proposes_ambiguous_matches_until_resolved() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let rent_id = rent(&pool, checking).await;
        let first = enter(&pool, checking, "2026-03-04", -80_000).await;
        let second = enter(&pool, checking, "2026-03-07", -80_000).await;
        let occurrence = date("2026-03-05");

        assert_eq!(
            match_certain(&pool, date("2026-03-20"), None)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            proposed(&pool, "2026-03-20").await,
            [(occurrence, first), (occurrence, second)]
        );

        // Dismissing one leaves a certain match.
        dismiss(&pool, rent_id, occurrence, first).await.unwrap();
        assert!(proposed(&pool, "2026-03-20").await.is_empty());
        assert_eq!(
            match_certain(&pool, date("2026-03-20"), None)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            realized(&pool, "2026-03-01", "2026-03-31").await,
            [(occurrence, second)]
        );
    });
}

#[test]
fn matches_by_hand_with_checks() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let other = new_account(&pool, "Autre").await;
        let rent_id = rent(&pool, checking).await;
        let far = enter(&pool, checking, "2026-03-15", -95_000).await;
        let close = enter(&pool, checking, "2026-03-06", -10_000).await;
        enter(&pool, checking, "2026-03-05", 80_000).await;
        enter(&pool, checking, "2026-03-25", -80_000).await;
        let elsewhere = enter(&pool, other, "2026-03-05", -80_000).await;
        let occurrence = date("2026-03-05");

        // Same direction, any amount, up to 15 days apart, the closest first.
        let ids: Vec<i64> = match_candidates(&pool, rent_id, occurrence)
            .await
            .unwrap()
            .iter()
            .map(|o| o.operation_id)
            .collect();
        assert_eq!(ids, [close, far]);

        assert!(matches!(
            match_occurrence(&pool, rent_id, date("2026-03-06"), close).await,
            Err(AppError::Schedule(ScheduleError::NotAnOccurrence(_)))
        ));
        assert!(matches!(
            match_occurrence(&pool, rent_id, occurrence, elsewhere).await,
            Err(AppError::Schedule(ScheduleError::OtherAccount))
        ));
        match_occurrence(&pool, rent_id, occurrence, far)
            .await
            .unwrap();
        assert!(matches!(
            match_occurrence(&pool, rent_id, occurrence, close).await,
            Err(AppError::Schedule(ScheduleError::AlreadyRealized))
        ));
        assert!(matches!(
            match_occurrence(&pool, rent_id, date("2026-04-05"), far).await,
            Err(AppError::Schedule(ScheduleError::OperationAlreadyMatched(id))) if id == far
        ));

        unmatch(&pool, rent_id, occurrence).await.unwrap();
        assert!(realized(&pool, "2026-03-01", "2026-03-31").await.is_empty());
    });
}

#[test]
fn forgets_matches_made_invalid() {
    run(|pool| async move {
        let checking = new_account(&pool, "Courant").await;
        let rent_id = rent(&pool, checking).await;
        let march = enter(&pool, checking, "2026-03-05", -80_000).await;
        let april = enter(&pool, checking, "2026-04-05", -80_000).await;
        match_occurrence(&pool, rent_id, date("2026-03-05"), march)
            .await
            .unwrap();
        match_occurrence(&pool, rent_id, date("2026-04-05"), april)
            .await
            .unwrap();

        // A deleted operation no longer realizes its occurrence.
        operation::delete(&pool, april).await.unwrap();
        assert_eq!(
            realized(&pool, "2026-01-01", "2026-12-31").await,
            [(date("2026-03-05"), march)]
        );

        // The rent moves to the 10th: the 5th is no longer an occurrence.
        schedule::update(&pool, rent_id, rent_input(checking, 10))
            .await
            .unwrap();
        assert!(realized(&pool, "2026-01-01", "2026-12-31").await.is_empty());
    });
}
