use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::operation::{self, NewOperation};

const GROCERIES: i64 = 1;
const RESTAURANTS: i64 = 2;
const TRANSPORT: i64 = 3;
const SALARY: i64 = 4;

/// Anonymized labels of a few months of a checking account.
fn examples() -> Vec<Example> {
    let debit = |label: &str, category_id| Example {
        label: label.to_owned(),
        is_credit: false,
        category_id,
    };
    let mut examples: Vec<Example> = [
        "CARREFOUR MKT",
        "CARREFOUR CITY",
        "CARREFOUR MARKET PARIS",
        "LIDL",
        "LIDL FRANCE",
        "AUCHAN SUPERMARCHE",
        "INTERMARCHE",
        "MONOPRIX",
        "FRANPRIX",
        "E LECLERC",
        "CASINO SHOP",
        "BIOCOOP",
    ]
    .into_iter()
    .map(|l| debit(l, GROCERIES))
    .collect();
    examples.extend(
        [
            "MCDONALDS",
            "BURGER KING",
            "SUSHI SHOP",
            "PIZZA HUT",
            "SC-M SUSHI PARIS",
            "RESTAURANT LE PETIT ZINC",
            "BRASSERIE DU MARCHE",
            "KFC",
            "DELIVEROO",
            "UBER EATS",
        ]
        .into_iter()
        .map(|l| debit(l, RESTAURANTS)),
    );
    examples.extend(
        [
            "SNCF",
            "SNCF INTERNET",
            "RATP",
            "NAVIGO",
            "TOTAL ACCESS",
            "TOTALENERGIES",
            "BLABLACAR",
            "PARKING INDIGO",
        ]
        .into_iter()
        .map(|l| debit(l, TRANSPORT)),
    );
    examples.extend(["ACME SALAIRE", "ACME PRIME"].into_iter().map(|l| Example {
        label: l.to_owned(),
        is_credit: true,
        category_id: SALARY,
    }));
    examples
}

#[test]
fn needs_enough_labels_and_two_categories() {
    let examples = examples();
    assert!(Classifier::train(&examples[..MIN_LABELS - 1]).is_none());
    let single: Vec<Example> = examples
        .iter()
        .map(|e| Example {
            category_id: GROCERIES,
            ..e.clone()
        })
        .collect();
    assert!(Classifier::train(&single).is_none());
    assert!(Classifier::train(&examples).is_some());
}

#[test]
fn suggests_the_category_of_close_labels() {
    let classifier = Classifier::train(&examples()).unwrap();
    let suggest =
        |label: &str, is_credit| classifier.suggest(label, is_credit).map(|s| s.category_id);
    assert_eq!(suggest("CARREFOUR EXPRESS", false), Some(GROCERIES));
    assert_eq!(suggest("SUSHI WOK", false), Some(RESTAURANTS));
    assert_eq!(suggest("SNCF VOYAGES", false), Some(TRANSPORT));
    assert_eq!(suggest("ACME SALAIRE OCTOBRE", true), Some(SALARY));
    let suggestion = classifier.suggest("CARREFOUR EXPRESS", false).unwrap();
    assert!((50..=100).contains(&suggestion.confidence_percent));
}

#[test]
fn gives_no_suggestion_below_the_confidence_threshold() {
    let classifier = Classifier::train(&examples()).unwrap();
    // No known term: only the prior of the categories, none above 50 %.
    assert_eq!(classifier.suggest("QWZX", false), None);
    assert_eq!(classifier.suggest("", false), None);
}

#[test]
fn terms_are_words_and_their_trigrams() {
    assert_eq!(terms("KFC 75 A"), ["w:KFC", "c: KF", "c:KFC", "c:FC "]);
}

fn date(text: &str) -> chrono::NaiveDate {
    text.parse().unwrap()
}

#[test]
fn learns_the_most_frequent_category_of_each_label_and_retrains_on_changes() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = account::create(
            &pool,
            NewAccount {
                name: "Courant".to_owned(),
                account_type: AccountType::Checking,
                institution: None,
                opening_balance_cents: 0,
                opening_date: date("2026-01-01"),
            },
        )
        .await
        .unwrap()
        .id;
        let categories = operation::category::list(&pool).await.unwrap();
        let find = |name: &str| categories.iter().find(|c| c.name == name).unwrap().id;
        let (groceries, restaurants) = (find("Courses"), find("Restaurants"));
        let enter = |label: &str, amount_cents: i64, category_id: Option<i64>| {
            let pool = pool.clone();
            let label = label.to_owned();
            async move {
                operation::create(
                    &pool,
                    NewOperation {
                        account_id,
                        date: date("2026-10-01"),
                        value_date: None,
                        amount_cents,
                        label,
                        category_id,
                        note: None,
                        tags: Vec::new(),
                    },
                )
                .await
                .unwrap();
            }
        };
        enter("LIDL", -1_000, Some(groceries)).await;
        enter("LIDL", -2_000, Some(groceries)).await;
        enter("LIDL", -3_000, Some(restaurants)).await;
        enter("LIDL", 500, Some(restaurants)).await;
        enter("KFC", -900, None).await;

        let found = super::examples(&pool).await.unwrap();
        let summary: Vec<(&str, bool, i64)> = found
            .iter()
            .map(|e| (e.label.as_str(), e.is_credit, e.category_id))
            .collect();
        assert_eq!(
            summary,
            [("LIDL", false, groceries), ("LIDL", true, restaurants)]
        );

        let cache = ClassifierCache::default();
        assert!(cache.refresh(&pool).await.unwrap().is_none());
        for index in 0..MIN_LABELS {
            let category_id = if index % 2 == 0 {
                groceries
            } else {
                restaurants
            };
            let name: String = [b'A' + (index / 26) as u8, b'A' + (index % 26) as u8]
                .iter()
                .map(|b| *b as char)
                .collect();
            enter(&format!("MAGASIN {name}"), -1_000, Some(category_id)).await;
        }
        // The last classifier until the new one is trained.
        assert!(cache.get(&pool).await.unwrap().is_none());
        assert!(cache.refresh(&pool).await.unwrap().is_some());
        assert!(cache.get(&pool).await.unwrap().is_some());
    });
}
