use sqlx::SqlitePool;

use super::{Account, AccountError, AccountType, AccountUpdate, NewAccount};
use crate::currency::{self, CurrencyError};
use crate::error::AppResult;

/// Lists accounts ordered by type then name. Archived accounts are included on demand.
pub async fn list(pool: &SqlitePool, include_archived: bool) -> AppResult<Vec<Account>> {
    let accounts = sqlx::query_as!(
        Account,
        r#"SELECT id, name, type AS "account_type: AccountType", currency, institution,
                  opening_balance_cents, opening_date AS "opening_date: _", archived AS "archived: bool"
           FROM account
           WHERE archived = 0 OR ?1
           ORDER BY CASE type
                        WHEN 'CHECKING' THEN 1 WHEN 'SAVINGS' THEN 2 WHEN 'CREDIT_CARD' THEN 3
                        WHEN 'INVESTMENT' THEN 4 WHEN 'ASSET' THEN 5 ELSE 6
                    END,
                    name COLLATE NOCASE"#,
        include_archived
    )
    .fetch_all(pool)
    .await?;
    Ok(accounts)
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Account> {
    let account = sqlx::query_as!(
        Account,
        r#"SELECT id, name, type AS "account_type: AccountType", currency, institution,
                  opening_balance_cents, opening_date AS "opening_date: _", archived AS "archived: bool"
           FROM account
           WHERE id = ?1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(account.ok_or(AccountError::NotFound(id))?)
}

pub async fn create(pool: &SqlitePool, new: NewAccount) -> AppResult<Account> {
    let name = clean_name(&new.name)?;
    let institution = clean_optional(new.institution);
    let currency = currency::check_supported(&new.currency)?;
    let id = sqlx::query!(
        "INSERT INTO account (name, type, currency, institution, opening_balance_cents, opening_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        name,
        new.account_type,
        currency,
        institution,
        new.opening_balance_cents,
        new.opening_date
    )
    .execute(pool)
    .await?
    .last_insert_rowid();
    get(pool, id).await
}

pub async fn update(pool: &SqlitePool, id: i64, changes: AccountUpdate) -> AppResult<Account> {
    let name = clean_name(&changes.name)?;
    let institution = clean_optional(changes.institution);
    let currency = currency::check_supported(&changes.currency)?;
    if currency != get(pool, id).await?.currency && has_data(pool, id).await? {
        return Err(CurrencyError::AccountCurrencyLocked.into());
    }
    sqlx::query!(
        "UPDATE account
         SET name = ?2, currency = ?3, institution = ?4, opening_balance_cents = ?5,
             opening_date = ?6
         WHERE id = ?1",
        id,
        name,
        currency,
        institution,
        changes.opening_balance_cents,
        changes.opening_date
    )
    .execute(pool)
    .await?;
    get(pool, id).await
}

/// Whether the account has operations, valuations, portfolio transactions or schedules: its
/// amounts are then in its currency.
async fn has_data(pool: &SqlitePool, id: i64) -> AppResult<bool> {
    let has_data = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM operation WHERE account_id = ?1)
               OR EXISTS (SELECT 1 FROM valuation WHERE account_id = ?1)
               OR EXISTS (SELECT 1 FROM investment_transaction WHERE account_id = ?1)
               OR EXISTS (SELECT 1 FROM scheduled_operation WHERE account_id = ?1)
               AS "has_data!: bool""#,
        id
    )
    .fetch_one(pool)
    .await?;
    Ok(has_data)
}

pub async fn set_archived(pool: &SqlitePool, id: i64, archived: bool) -> AppResult<Account> {
    let updated = sqlx::query!(
        "UPDATE account SET archived = ?2 WHERE id = ?1",
        id,
        archived
    )
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(AccountError::NotFound(id).into());
    }
    get(pool, id).await
}

fn clean_name(name: &str) -> Result<String, AccountError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AccountError::EmptyName);
    }
    Ok(name.to_owned())
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::db::tests::memory_pool;
    use crate::error::AppError;

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    fn new_account(name: &str, account_type: AccountType) -> NewAccount {
        NewAccount {
            name: name.to_owned(),
            account_type,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: date("2026-01-01"),
        }
    }

    #[test]
    fn creates_and_reads_back_an_account() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let created = create(
                &pool,
                NewAccount {
                    name: "  Compte courant ".to_owned(),
                    account_type: AccountType::Checking,
                    currency: "EUR".to_owned(),
                    institution: Some(" Ma banque ".to_owned()),
                    opening_balance_cents: 41323,
                    opening_date: date("2026-01-01"),
                },
            )
            .await
            .unwrap();

            assert_eq!(created.name, "Compte courant");
            assert_eq!(created.account_type, AccountType::Checking);
            assert_eq!(created.currency, "EUR");
            assert_eq!(created.institution.as_deref(), Some("Ma banque"));
            assert_eq!(created.opening_balance_cents, 41323);
            assert_eq!(created.opening_date, date("2026-01-01"));
            assert!(!created.archived);
            assert_eq!(get(&pool, created.id).await.unwrap(), created);
        });
    }

    #[test]
    fn rejects_an_empty_name() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let result = create(&pool, new_account("   ", AccountType::Savings)).await;
            assert!(matches!(
                result,
                Err(AppError::Account(AccountError::EmptyName))
            ));
        });
    }

    #[test]
    fn updates_editable_fields() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let account = create(&pool, new_account("Livret", AccountType::Savings))
                .await
                .unwrap();

            let updated = update(
                &pool,
                account.id,
                AccountUpdate {
                    name: "Livret A".to_owned(),
                    currency: "EUR".to_owned(),
                    institution: Some("   ".to_owned()),
                    opening_balance_cents: 1_500_000,
                    opening_date: date("2025-06-01"),
                },
            )
            .await
            .unwrap();

            assert_eq!(updated.name, "Livret A");
            assert_eq!(updated.institution, None);
            assert_eq!(updated.opening_balance_cents, 1_500_000);
            assert_eq!(updated.opening_date, date("2025-06-01"));
            assert_eq!(updated.account_type, AccountType::Savings);
        });
    }

    #[test]
    fn reports_unknown_accounts() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            assert!(matches!(
                get(&pool, 42).await,
                Err(AppError::Account(AccountError::NotFound(42)))
            ));
            assert!(matches!(
                set_archived(&pool, 42, true).await,
                Err(AppError::Account(AccountError::NotFound(42)))
            ));
        });
    }

    #[test]
    fn hides_archived_accounts_unless_requested() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let old = create(&pool, new_account("Ancien compte", AccountType::Checking))
                .await
                .unwrap();
            create(&pool, new_account("Compte courant", AccountType::Checking))
                .await
                .unwrap();
            set_archived(&pool, old.id, true).await.unwrap();

            let names = |accounts: Vec<Account>| -> Vec<String> {
                accounts.into_iter().map(|a| a.name).collect()
            };
            assert_eq!(names(list(&pool, false).await.unwrap()), ["Compte courant"]);
            assert_eq!(
                names(list(&pool, true).await.unwrap()),
                ["Ancien compte", "Compte courant"]
            );
        });
    }

    #[test]
    fn lists_accounts_by_type_then_name() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            for (name, account_type) in [
                ("Prêt immobilier", AccountType::Liability),
                ("PEA", AccountType::Investment),
                ("LDDS", AccountType::Savings),
                ("Compte courant", AccountType::Checking),
                ("Appartement", AccountType::Asset),
                ("Livret A", AccountType::Savings),
            ] {
                create(&pool, new_account(name, account_type))
                    .await
                    .unwrap();
            }

            let names: Vec<String> = list(&pool, false)
                .await
                .unwrap()
                .into_iter()
                .map(|a| a.name)
                .collect();
            assert_eq!(
                names,
                [
                    "Compte courant",
                    "LDDS",
                    "Livret A",
                    "PEA",
                    "Appartement",
                    "Prêt immobilier"
                ]
            );
        });
    }
}
