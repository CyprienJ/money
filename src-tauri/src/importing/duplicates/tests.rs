use sqlx::SqlitePool;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::operation::{self, NewOperation};

use AmbiguityReason::*;
use DuplicateStatus::*;

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn line(day: &str, amount_cents: i64, label: &str) -> RawLine {
    RawLine {
        date: date(day),
        value_date: None,
        label: label.to_owned(),
        amount_cents,
        note: None,
        source_category: None,
    }
}

fn imported(id: i64, day: &str, amount_cents: i64, label: &str) -> ExistingOperation {
    ExistingOperation {
        id,
        date: date(day),
        amount_cents,
        raw_label: label.to_owned(),
        is_manual: false,
    }
}

fn manual(id: i64, day: &str, amount_cents: i64, label: &str) -> ExistingOperation {
    ExistingOperation {
        is_manual: true,
        ..imported(id, day, amount_cents, label)
    }
}

fn ambiguous(reason: AmbiguityReason, candidate_ids: &[i64]) -> DuplicateStatus {
    Ambiguous {
        reason,
        candidate_ids: candidate_ids.to_vec(),
    }
}

#[test]
fn empty_account_makes_every_line_new() {
    let lines = [
        line("2026-09-02", -420, "CB BOULANGERIE"),
        line("2026-09-02", -420, "CB BOULANGERIE"),
    ];
    assert_eq!(classify(&lines, &[]), vec![New, New]);
}

#[test]
fn exact_match_is_a_duplicate_whatever_its_origin() {
    let existing = [
        imported(1, "2026-09-02", -420, "CB BOULANGERIE"),
        manual(2, "2026-09-03", 245_000, "VIR SALAIRE"),
    ];
    let lines = [
        line("2026-09-02", -420, "CB BOULANGERIE"),
        line("2026-09-03", 245_000, "VIR SALAIRE"),
    ];
    assert_eq!(
        classify(&lines, &existing),
        vec![Duplicate { operation_id: 1 }, Duplicate { operation_id: 2 }]
    );
}

#[test]
fn label_comparison_is_exact() {
    let existing = [imported(1, "2026-09-02", -420, "CB BOULANGERIE")];
    let lines = [line("2026-09-02", -420, "CB Boulangerie")];
    assert_eq!(
        classify(&lines, &existing),
        vec![ambiguous(SameDayOtherLabel, &[1])]
    );
}

#[test]
fn identical_lines_are_matched_one_to_one() {
    let existing = [
        imported(1, "2026-09-02", -420, "CB BOULANGERIE"),
        imported(2, "2026-09-02", -420, "CB BOULANGERIE"),
    ];
    let lines = [
        line("2026-09-02", -420, "CB BOULANGERIE"),
        line("2026-09-02", -420, "CB BOULANGERIE"),
        line("2026-09-02", -420, "CB BOULANGERIE"),
    ];
    assert_eq!(
        classify(&lines, &existing),
        vec![
            Duplicate { operation_id: 1 },
            Duplicate { operation_id: 2 },
            ambiguous(RepeatedLine, &[1, 2]),
        ]
    );
}

#[test]
fn same_day_and_amount_with_another_label_is_ambiguous() {
    let existing = [
        imported(1, "2026-09-02", -1349, "PRLV NETFLIX"),
        imported(2, "2026-09-02", -1349, "CB CINEMA"),
    ];
    let lines = [line("2026-09-02", -1349, "NETFLIX.COM")];
    assert_eq!(
        classify(&lines, &existing),
        vec![ambiguous(SameDayOtherLabel, &[1, 2])]
    );
}

#[test]
fn operation_matched_exactly_is_not_a_candidate_again() {
    let existing = [imported(1, "2026-09-02", -1349, "PRLV NETFLIX")];
    let lines = [
        line("2026-09-02", -1349, "NETFLIX.COM"),
        line("2026-09-02", -1349, "PRLV NETFLIX"),
    ];
    assert_eq!(
        classify(&lines, &existing),
        vec![New, Duplicate { operation_id: 1 }]
    );
}

#[test]
fn manual_entry_within_three_days_is_ambiguous() {
    let existing = [
        manual(1, "2026-09-05", -3500, "Restaurant avec Paul"),
        manual(2, "2026-09-02", -3500, "Coiffeur"),
    ];
    let lines = [line("2026-09-08", -3500, "CB LE BISTROT 05/09")];
    assert_eq!(
        classify(&lines, &existing),
        vec![ambiguous(NearManualEntry, &[1])]
    );
}

#[test]
fn manual_entry_beyond_three_days_or_other_amount_is_ignored() {
    let existing = [
        manual(1, "2026-09-01", -3500, "Restaurant"),
        manual(2, "2026-09-05", -3600, "Restaurant"),
    ];
    let lines = [line("2026-09-05", -3500, "CB LE BISTROT")];
    assert_eq!(classify(&lines, &existing), vec![New]);
}

#[test]
fn imported_operation_a_few_days_apart_is_not_ambiguous() {
    let existing = [imported(1, "2026-08-05", -1349, "PRLV NETFLIX")];
    let lines = [line("2026-08-06", -1349, "PRLV NETFLIX")];
    assert_eq!(classify(&lines, &existing), vec![New]);
}

#[test]
fn only_new_lines_are_included_by_default() {
    assert!(New.included_by_default());
    assert!(!Duplicate { operation_id: 1 }.included_by_default());
    assert!(!ambiguous(RepeatedLine, &[1]).included_by_default());
}

#[test]
fn loads_operations_of_the_account_around_the_file_period() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = new_account(&pool, "Courant").await;
        let other_id = new_account(&pool, "Livret").await;
        let before = enter(&pool, account_id, "2026-08-28", -100).await;
        let inside = enter(&pool, account_id, "2026-09-10", -200).await;
        let after = enter(&pool, account_id, "2026-09-23", -300).await;
        enter(&pool, account_id, "2026-08-27", -400).await;
        enter(&pool, account_id, "2026-09-24", -500).await;
        enter(&pool, other_id, "2026-09-10", -200).await;
        let batch_id = import_batch(&pool, account_id).await;
        let imported_id = sqlx::query(
            "INSERT INTO operation (account_id, date, amount_cents, raw_label, normalized_label,
                                    import_batch_id, created_at, updated_at)
             VALUES (?1, '2026-09-15', -600, 'CB X', 'X', ?2, '', '')",
        )
        .bind(account_id)
        .bind(batch_id)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();

        let lines = [line("2026-09-20", -1, "A"), line("2026-08-31", -1, "B")];
        let existing = existing_operations(&pool, account_id, &lines)
            .await
            .unwrap();
        let found: Vec<(i64, bool)> = existing
            .iter()
            .map(|operation| (operation.id, operation.is_manual))
            .collect();
        assert_eq!(
            found,
            vec![
                (before, true),
                (inside, true),
                (imported_id, false),
                (after, true)
            ]
        );
        assert_eq!(existing[1].raw_label, "Op -200");

        assert!(
            existing_operations(&pool, account_id, &[])
                .await
                .unwrap()
                .is_empty()
        );
    });
}

async fn new_account(pool: &SqlitePool, name: &str) -> i64 {
    account::create(
        pool,
        NewAccount {
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

async fn enter(pool: &SqlitePool, account_id: i64, day: &str, amount_cents: i64) -> i64 {
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: date(day),
            value_date: None,
            amount_cents,
            label: format!("Op {amount_cents}"),
            category_id: None,
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
    .id
}

async fn import_batch(pool: &SqlitePool, account_id: i64) -> i64 {
    sqlx::query(
        "INSERT INTO import_batch (account_id, source_type, source_file_name, source_file_hash,
                                   imported_at, balance_check_status)
         VALUES (?1, 'CSV', 'releve.csv', 'hash', '', 'NOT_AVAILABLE')",
    )
    .bind(account_id)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}
