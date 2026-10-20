//! Tests of the constraints declared in the SQL migrations.

use sqlx::{AssertSqlSafe, SqlitePool};

use super::tests::memory_pool;

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        test(pool).await;
    });
}

// Test-only SQL built from constants and literals, never from user input.
async fn exec(pool: &SqlitePool, sql: &str) -> Result<(), sqlx::Error> {
    sqlx::query(AssertSqlSafe(sql.to_owned()))
        .execute(pool)
        .await
        .map(|_| ())
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(AssertSqlSafe(sql.to_owned()))
        .fetch_one(pool)
        .await
        .unwrap()
}

const INSERT_ACCOUNT: &str = "INSERT INTO account (id, name, type, opening_date)
     VALUES (1, 'Compte courant', 'CHECKING', '2026-01-01')";

const INSERT_BATCH: &str = "INSERT INTO import_batch
     (id, account_id, source_type, source_file_name, source_file_hash, imported_at, balance_check_status)
     VALUES (1, 1, 'CSV', 'releve.csv', 'abc', '2026-10-19T10:00:00Z', 'NOT_AVAILABLE')";

const INSERT_IMPORTED_OPERATION: &str = "INSERT INTO operation
     (id, account_id, date, amount_cents, raw_label, normalized_label, import_batch_id, created_at, updated_at)
     VALUES (1, 1, '2026-10-19', -6428, 'CB CARREFOUR MKT 18/10', 'CARREFOUR MKT', 1,
             '2026-10-19T10:00:00Z', '2026-10-19T10:00:00Z')";

const INSERT_SPLIT: &str =
    "INSERT INTO operation_split (operation_id, category_id, amount_cents) VALUES (1, NULL, -6428)";

#[test]
fn creates_all_v1_tables() {
    run(|pool| async move {
        let tables = count(
            &pool,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                'account', 'operation', 'operation_split', 'category', 'import_batch',
                'csv_mapping', 'label_category_memory', 'valuation', 'scheduled_operation')",
        )
        .await;
        assert_eq!(tables, 9);
    });
}

#[test]
fn seeds_a_single_internal_transfer_category() {
    run(|pool| async move {
        let seeded = count(
            &pool,
            "SELECT COUNT(*) FROM category WHERE is_internal_transfer = 1 AND name = 'Virement interne'",
        )
        .await;
        assert_eq!(seeded, 1);

        let second = exec(
            &pool,
            "INSERT INTO category (name, is_internal_transfer) VALUES ('Autre virement', 1)",
        )
        .await;
        assert!(second.is_err());
    });
}

#[test]
fn deleting_a_batch_removes_its_operations_and_splits() {
    run(|pool| async move {
        for sql in [
            INSERT_ACCOUNT,
            INSERT_BATCH,
            INSERT_IMPORTED_OPERATION,
            INSERT_SPLIT,
        ] {
            exec(&pool, sql).await.unwrap();
        }

        exec(&pool, "DELETE FROM import_batch WHERE id = 1")
            .await
            .unwrap();

        assert_eq!(count(&pool, "SELECT COUNT(*) FROM operation").await, 0);
        assert_eq!(
            count(&pool, "SELECT COUNT(*) FROM operation_split").await,
            0
        );
    });
}

#[test]
fn refuses_to_delete_an_account_with_operations() {
    run(|pool| async move {
        for sql in [INSERT_ACCOUNT, INSERT_BATCH, INSERT_IMPORTED_OPERATION] {
            exec(&pool, sql).await.unwrap();
        }

        assert!(
            exec(&pool, "DELETE FROM account WHERE id = 1")
                .await
                .is_err()
        );
    });
}

#[test]
fn rejects_invalid_enum_values_and_dates() {
    run(|pool| async move {
        let bad_type = exec(
            &pool,
            "INSERT INTO account (name, type, opening_date) VALUES ('X', 'WALLET', '2026-01-01')",
        )
        .await;
        assert!(bad_type.is_err());

        let bad_date = exec(
            &pool,
            "INSERT INTO account (name, type, opening_date) VALUES ('X', 'CHECKING', '01/01/2026')",
        )
        .await;
        assert!(bad_date.is_err());

        let bad_color = exec(
            &pool,
            "INSERT INTO category (name, color) VALUES ('X', 'pink')",
        )
        .await;
        assert!(bad_color.is_err());
    });
}

#[test]
fn category_names_are_unique_among_siblings() {
    run(|pool| async move {
        exec(
            &pool,
            "INSERT INTO category (id, name) VALUES (100, 'Parent de test')",
        )
        .await
        .unwrap();
        assert!(
            exec(
                &pool,
                "INSERT INTO category (name) VALUES ('Parent de test')"
            )
            .await
            .is_err()
        );

        exec(
            &pool,
            "INSERT INTO category (name, parent_id) VALUES ('Loyer', 100)",
        )
        .await
        .unwrap();
        assert!(
            exec(
                &pool,
                "INSERT INTO category (name, parent_id) VALUES ('Loyer', 100)"
            )
            .await
            .is_err()
        );
    });
}

#[test]
fn csv_mapping_requires_an_amount_or_debit_and_credit_columns() {
    run(|pool| async move {
        let insert = |columns: &str| {
            format!(
                "INSERT INTO csv_mapping (name, delimiter, encoding, date_format, decimal_separator,
                 column_date, column_label, column_amount, column_debit, column_credit)
                 VALUES ('{columns}', ';', 'windows-1252', '%d/%m/%Y', ',', 0, 1, {columns})"
            )
        };

        assert!(exec(&pool, &insert("NULL, 3, NULL")).await.is_err());
        exec(&pool, &insert("2, NULL, NULL")).await.unwrap();
        exec(&pool, &insert("NULL, 2, 3")).await.unwrap();
    });
}

#[test]
fn schedule_end_date_cannot_precede_start_date() {
    run(|pool| async move {
        exec(&pool, INSERT_ACCOUNT).await.unwrap();
        let result = exec(
            &pool,
            "INSERT INTO scheduled_operation
             (account_id, label, amount_cents, frequency, day_of_period, start_date, end_date)
             VALUES (1, 'Loyer', -84200, 'MONTHLY', 5, '2026-01-01', '2025-12-31')",
        )
        .await;
        assert!(result.is_err());
    });
}
