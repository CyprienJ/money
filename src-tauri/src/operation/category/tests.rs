use chrono::NaiveDate;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::error::AppError;
use crate::operation::{self, NewOperation};

fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
    tauri::async_runtime::block_on(async {
        test(memory_pool().await).await;
    });
}

fn top(name: &str, color: CategoryColor) -> CategoryInput {
    CategoryInput {
        name: name.to_owned(),
        parent_id: None,
        color,
        is_income: false,
    }
}

fn child(name: &str, parent_id: i64) -> CategoryInput {
    CategoryInput {
        name: name.to_owned(),
        parent_id: Some(parent_id),
        // Ignored for a subcategory.
        color: CategoryColor::Health,
        is_income: false,
    }
}

async fn by_name(pool: &SqlitePool, name: &str, parent: Option<i64>) -> Category {
    list(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.name == name && c.parent_id == parent)
        .unwrap_or_else(|| panic!("no category {name}"))
}

async fn internal_transfer(pool: &SqlitePool) -> Category {
    list(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.is_internal_transfer)
        .unwrap()
}

async fn get(pool: &SqlitePool, id: i64) -> AppResult<Category> {
    let mut conn = pool.acquire().await?;
    fetch(&mut conn, id).await
}

fn category_error(result: AppResult<impl std::fmt::Debug>) -> CategoryError {
    match result {
        Err(AppError::Category(error)) => error,
        other => panic!("expected a category error, got {other:?}"),
    }
}

/// Creates an account and an operation in `category_id`; returns the operation id.
async fn operation_in(pool: &SqlitePool, category_id: i64) -> i64 {
    let account_id = account::create(
        pool,
        NewAccount {
            is_joint: false,
            name: format!("Compte {category_id}"),
            account_type: AccountType::Checking,
            currency: "EUR".to_owned(),
            institution: None,
            opening_balance_cents: 0,
            opening_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        },
    )
    .await
    .unwrap()
    .id;
    operation::create(
        pool,
        NewOperation {
            account_id,
            date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            value_date: None,
            amount_cents: -1000,
            label: "Op".to_owned(),
            category_id: Some(category_id),
            splits: Vec::new(),
            note: None,
            tags: Vec::new(),
        },
    )
    .await
    .unwrap()
    .id
}

#[test]
fn creates_the_default_tree_once() {
    run(|pool| async move {
        let categories = list(&pool).await.unwrap();
        let tops: Vec<&str> = categories
            .iter()
            .filter(|c| c.parent_id.is_none())
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(
            tops,
            [
                "Abonnements",
                "Autres",
                "Courses",
                "Logement",
                "Loisirs",
                "Restaurants",
                "Revenus",
                "Santé",
                "Transport",
                "Virement interne"
            ]
        );
        assert_eq!(categories.len(), 10 + 33);

        let income = by_name(&pool, "Revenus", None).await;
        let salary = by_name(&pool, "Salaire", Some(income.id)).await;
        assert!(income.is_income && salary.is_income);
        assert_eq!(salary.color, CategoryColor::Transport);
        let restaurants = by_name(&pool, "Restaurants", None).await;
        let nested = by_name(&pool, "Restaurants", Some(restaurants.id)).await;
        assert_eq!(nested.color, CategoryColor::Restaurants);
        assert!(!nested.is_income);
    });
}

#[test]
fn subcategories_inherit_color_and_income_flag() {
    run(|pool| async move {
        let parent = create(
            &pool,
            CategoryInput {
                is_income: true,
                ..top("Aides", CategoryColor::Leisure)
            },
        )
        .await
        .unwrap();
        let sub = create(&pool, child("  CAF  ", parent.id)).await.unwrap();
        assert_eq!(sub.name, "CAF");
        assert_eq!(sub.parent_id, Some(parent.id));
        assert_eq!(sub.color, CategoryColor::Leisure);
        assert!(sub.is_income);
        assert!(!sub.is_internal_transfer);

        // Changing the parent changes its subcategories.
        update(&pool, parent.id, top("Aides", CategoryColor::Housing))
            .await
            .unwrap();
        let sub = get(&pool, sub.id).await.unwrap();
        assert_eq!(sub.color, CategoryColor::Housing);
        assert!(!sub.is_income);

        // A subcategory moved elsewhere takes its new parent's values.
        let income = by_name(&pool, "Revenus", None).await;
        let moved = update(&pool, sub.id, child("CAF", income.id))
            .await
            .unwrap();
        assert_eq!(moved.color, CategoryColor::Transport);
        assert!(moved.is_income);

        // And becomes top-level with its own values.
        let promoted = update(&pool, sub.id, top("CAF", CategoryColor::Health))
            .await
            .unwrap();
        assert_eq!(
            (promoted.parent_id, promoted.color),
            (None, CategoryColor::Health)
        );
    });
}

#[test]
fn validates_names() {
    run(|pool| async move {
        assert!(matches!(
            category_error(create(&pool, top(" ", CategoryColor::Neutral)).await),
            CategoryError::EmptyName
        ));
        assert!(matches!(
            category_error(create(&pool, top("courses", CategoryColor::Neutral)).await),
            CategoryError::DuplicateName(name) if name == "courses"
        ));
        let groceries = by_name(&pool, "Courses", None).await;
        assert!(matches!(
            category_error(create(&pool, child("ALIMENTATION", groceries.id)).await),
            CategoryError::DuplicateName(_)
        ));
        // The same name under another parent is allowed.
        let leisure = by_name(&pool, "Loisirs", None).await;
        create(&pool, child("Alimentation", leisure.id))
            .await
            .unwrap();
        // Keeping its own name is not a duplicate.
        update(
            &pool,
            groceries.id,
            top("Courses", CategoryColor::Groceries),
        )
        .await
        .unwrap();
    });
}

#[test]
fn keeps_two_levels() {
    run(|pool| async move {
        let groceries = by_name(&pool, "Courses", None).await;
        let food = by_name(&pool, "Alimentation", Some(groceries.id)).await;
        let transfer = internal_transfer(&pool).await;
        let leisure = by_name(&pool, "Loisirs", None).await;

        for parent in [food.id, transfer.id, 999] {
            assert!(matches!(
                category_error(create(&pool, child("X", parent)).await),
                CategoryError::InvalidParent
            ));
        }
        assert!(matches!(
            category_error(update(&pool, leisure.id, child("Loisirs", leisure.id)).await),
            CategoryError::HasSubcategories
        ));
        let empty = create(&pool, top("Vide", CategoryColor::Neutral))
            .await
            .unwrap();
        assert!(matches!(
            category_error(update(&pool, empty.id, child("Vide", empty.id)).await),
            CategoryError::InvalidParent
        ));
    });
}

#[test]
fn internal_transfer_category_is_locked() {
    run(|pool| async move {
        let transfer = internal_transfer(&pool).await;
        assert!(matches!(
            category_error(
                update(&pool, transfer.id, top("Virements", CategoryColor::Neutral)).await
            ),
            CategoryError::SystemCategoryLocked
        ));
        assert!(matches!(
            category_error(delete(&pool, transfer.id, None).await),
            CategoryError::SystemCategoryLocked
        ));
    });
}

#[test]
fn deleting_moves_operations_to_another_category() {
    run(|pool| async move {
        let groceries = by_name(&pool, "Courses", None).await;
        let food = by_name(&pool, "Alimentation", Some(groceries.id)).await;
        let home = by_name(&pool, "Maison et hygiène", Some(groceries.id)).await;
        let transfer = internal_transfer(&pool).await;

        let moved = operation_in(&pool, food.id).await;
        assert_eq!(get(&pool, food.id).await.unwrap().operation_count, 1);
        assert_eq!(get(&pool, home.id).await.unwrap().operation_count, 0);
        delete(&pool, food.id, Some(home.id)).await.unwrap();
        assert_eq!(get(&pool, home.id).await.unwrap().operation_count, 1);
        assert!(matches!(
            category_error(get(&pool, food.id).await),
            CategoryError::NotFound(_)
        ));
        assert_eq!(
            operation::get(&pool, moved).await.unwrap().category_id,
            Some(home.id)
        );

        delete(&pool, home.id, Some(transfer.id)).await.unwrap();
        let operation = operation::get(&pool, moved).await.unwrap();
        assert_eq!(operation.category_id, Some(transfer.id));
        assert!(operation.is_internal_transfer);

        let other = by_name(&pool, "Autres", None).await;
        let misc = by_name(&pool, "Divers", Some(other.id)).await;
        let cleared = operation_in(&pool, misc.id).await;
        delete(&pool, misc.id, None).await.unwrap();
        let operation = operation::get(&pool, cleared).await.unwrap();
        assert_eq!(operation.category_id, None);
        assert!(!operation.is_internal_transfer);
    });
}

#[test]
fn refuses_invalid_deletions() {
    run(|pool| async move {
        let groceries = by_name(&pool, "Courses", None).await;
        let food = by_name(&pool, "Alimentation", Some(groceries.id)).await;
        assert!(matches!(
            category_error(delete(&pool, groceries.id, None).await),
            CategoryError::HasSubcategories
        ));
        assert!(matches!(
            category_error(delete(&pool, food.id, Some(food.id)).await),
            CategoryError::ReassignToSelf
        ));
        assert!(matches!(
            category_error(delete(&pool, food.id, Some(999)).await),
            CategoryError::NotFound(999)
        ));
        assert!(matches!(
            category_error(delete(&pool, 999, None).await),
            CategoryError::NotFound(999)
        ));
        // Nothing changed.
        get(&pool, food.id).await.unwrap();
    });
}
