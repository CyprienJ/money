//! Category suggestion by a local classifier, for the labels no rule nor memory covers.
//!
//! Each distinct normalized label of the categorized operations, in each direction, is an
//! example of its most frequent category. Its features are the TF-IDF of its words and of the
//! character trigrams of its words (the IDF learned on the examples), plus the direction of
//! the amount; a multiclass logistic regression (linfa) learns the categories from them.
//!
//! The model lives in memory. [`ClassifierCache`] trains it again when the categorized
//! operations changed, and gives no classifier below `MIN_LABELS` examples or with a single
//! category. A suggestion under `MIN_CONFIDENCE` is not given, and none is applied without
//! the user's validation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use linfa::Dataset;
use linfa::traits::Fit;
use linfa_logistic::{MultiFittedLogisticRegression, MultiLogisticRegression};
use ndarray::{Array1, Array2};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use crate::error::AppResult;

/// Distinct categorized labels needed to train the classifier.
pub const MIN_LABELS: usize = 30;
/// Lowest probability of a suggestion.
pub const MIN_CONFIDENCE: f64 = 0.5;
/// Largest vocabulary, the most frequent terms first, to bound the memory of training.
const MAX_FEATURES: usize = 3_000;
/// L2 regularization of the logistic regression.
const ALPHA: f64 = 0.01;
const MAX_ITERATIONS: u64 = 200;

/// A category suggested for a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub category_id: i64,
    /// Probability of the category, in percent (50 to 100).
    pub confidence_percent: u8,
}

/// A categorized label to learn from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    pub label: String,
    pub is_credit: bool,
    pub category_id: i64,
}

pub struct Classifier {
    vectorizer: Vectorizer,
    model: MultiFittedLogisticRegression<f64, i64>,
}

/// TF-IDF of the terms learned from the examples.
struct Vectorizer {
    /// Column of each term.
    vocabulary: HashMap<String, usize>,
    /// Inverse document frequency of each column of the vocabulary.
    idf: Vec<f64>,
}

/// Words of a label (letters and digits), without single characters and pure numbers.
fn words(label: &str) -> impl Iterator<Item = &str> {
    label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 1 && !w.chars().all(|c| c.is_ascii_digit()))
}

/// Terms of a label: its words, and the character trigrams of each word padded with spaces,
/// so that "CARREFOUR MKT" and "CARREFOUR CITY" share their first word and "INTERMARCHE" and
/// "INTERMARCHÉ" most of their trigrams.
fn terms(label: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for word in words(label) {
        terms.push(format!("w:{word}"));
        let padded: Vec<char> = format!(" {word} ").chars().collect();
        terms.extend(
            padded
                .windows(3)
                .map(|t| format!("c:{}", t.iter().collect::<String>())),
        );
    }
    terms
}

impl Classifier {
    /// Trains on the examples; `None` below `MIN_LABELS` examples or with a single category.
    pub fn train(examples: &[Example]) -> Option<Classifier> {
        let mut categories: Vec<i64> = examples.iter().map(|e| e.category_id).collect();
        categories.sort_unstable();
        categories.dedup();
        if examples.len() < MIN_LABELS || categories.len() < 2 {
            return None;
        }

        let documents: Vec<Vec<String>> = examples.iter().map(|e| terms(&e.label)).collect();
        let mut document_frequency: HashMap<&str, usize> = HashMap::new();
        for document in &documents {
            let mut seen: Vec<&str> = document.iter().map(String::as_str).collect();
            seen.sort_unstable();
            seen.dedup();
            for term in seen {
                *document_frequency.entry(term).or_default() += 1;
            }
        }
        let mut kept: Vec<(&str, usize)> = document_frequency.into_iter().collect();
        // The most frequent terms first, then by text for a stable vocabulary.
        kept.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        kept.truncate(MAX_FEATURES);
        let count = examples.len() as f64;
        let vocabulary: HashMap<String, usize> = kept
            .iter()
            .enumerate()
            .map(|(column, (term, _))| ((*term).to_owned(), column))
            .collect();
        // Smooth IDF, as scikit-learn.
        let idf: Vec<f64> = kept
            .iter()
            .map(|(_, df)| ((1.0 + count) / (1.0 + *df as f64)).ln() + 1.0)
            .collect();

        let vectorizer = Vectorizer { vocabulary, idf };
        let mut records = Array2::zeros((examples.len(), vectorizer.width()));
        for (row, (example, document)) in examples.iter().zip(&documents).enumerate() {
            let features = vectorizer.features(document, example.is_credit);
            records.row_mut(row).assign(&features);
        }
        let targets: Array1<i64> = examples.iter().map(|e| e.category_id).collect();
        let model = MultiLogisticRegression::default()
            .alpha(ALPHA)
            .max_iterations(MAX_ITERATIONS)
            .fit(&Dataset::new(records, targets))
            .ok()?;
        Some(Classifier { vectorizer, model })
    }

    /// The most probable category of a label, when its probability reaches `MIN_CONFIDENCE`.
    pub fn suggest(&self, label: &str, is_credit: bool) -> Option<Suggestion> {
        let features = self.vectorizer.features(&terms(label), is_credit);
        let row = features.insert_axis(ndarray::Axis(0));
        let probabilities = self.model.predict_probabilities(&row);
        let (best, probability) = probabilities
            .row(0)
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))?;
        (probability >= MIN_CONFIDENCE).then(|| Suggestion {
            category_id: self.model.classes()[best],
            confidence_percent: (probability * 100.0).floor() as u8,
        })
    }
}

impl Vectorizer {
    /// Columns: the vocabulary, then the direction of the amount.
    fn width(&self) -> usize {
        self.idf.len() + 1
    }

    /// TF-IDF of the known terms, normalized to a unit length, and 1 for a credit.
    fn features(&self, terms: &[String], is_credit: bool) -> Array1<f64> {
        let mut features = Array1::<f64>::zeros(self.width());
        for term in terms {
            if let Some(&column) = self.vocabulary.get(term) {
                features[column] += self.idf[column];
            }
        }
        let norm = features.dot(&features).sqrt();
        if norm > 0.0 {
            features /= norm;
        }
        features[self.idf.len()] = if is_credit { 1.0 } else { 0.0 };
        features
    }
}

/// Distinct labels of the categorized operations by direction, each with its most frequent
/// category (the most recent one on a tie).
pub async fn examples(pool: &SqlitePool) -> AppResult<Vec<Example>> {
    let rows = sqlx::query!(
        r#"SELECT o.normalized_label AS "label!", o.amount_cents > 0 AS "is_credit!: bool",
                  s.category_id AS "category_id!", COUNT(*) AS "count!: i64",
                  MAX(o.date) AS "last_date!: String"
           FROM operation o
           JOIN operation_split s ON s.operation_id = o.id
           WHERE s.category_id IS NOT NULL AND o.normalized_label <> ''
           GROUP BY 1, 2, 3
           ORDER BY 1, 2, 4 DESC, 5 DESC"#
    )
    .fetch_all(pool)
    .await?;
    let mut examples: Vec<Example> = Vec::new();
    for row in rows {
        let same = examples
            .last()
            .is_some_and(|e| e.label == row.label && e.is_credit == row.is_credit);
        if !same {
            examples.push(Example {
                label: row.label,
                is_credit: row.is_credit,
                category_id: row.category_id,
            });
        }
    }
    Ok(examples)
}

/// A category suggested for an operation to categorize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationSuggestion {
    pub operation_id: i64,
    pub suggestion: Suggestion,
}

/// Suggestions for the operations of `operation_ids` that have no category.
pub async fn suggest_for_operations(
    pool: &SqlitePool,
    classifier: &Classifier,
    operation_ids: &[i64],
) -> AppResult<Vec<OperationSuggestion>> {
    let ids: Vec<String> = operation_ids.iter().map(i64::to_string).collect();
    let ids = format!("[{}]", ids.join(","));
    let rows = sqlx::query!(
        r#"SELECT o.id AS "id!", o.normalized_label, o.amount_cents
           FROM operation o
           JOIN operation_split s ON s.operation_id = o.id
           WHERE o.id IN (SELECT value FROM json_each(?1)) AND s.category_id IS NULL
           ORDER BY o.id"#,
        ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let suggestion = classifier.suggest(&row.normalized_label, row.amount_cents > 0)?;
            Some(OperationSuggestion {
                operation_id: row.id,
                suggestion,
            })
        })
        .collect())
}

/// Changes when an operation is categorized, recategorized or relabeled.
async fn fingerprint(pool: &SqlitePool) -> AppResult<(i64, i64, String)> {
    let row = sqlx::query!(
        r#"SELECT COUNT(*) AS "count!: i64",
                  COALESCE(SUM(s.category_id * o.id), 0) AS "checksum!: i64",
                  COALESCE(MAX(o.updated_at), '') AS "updated_at!: String"
           FROM operation o
           JOIN operation_split s ON s.operation_id = o.id
           WHERE s.category_id IS NOT NULL"#
    )
    .fetch_one(pool)
    .await?;
    Ok((row.count, row.checksum, row.updated_at))
}

type Fingerprint = (i64, i64, String);

#[derive(Default)]
struct CacheState {
    /// Fingerprint of the operations `classifier` was trained on.
    trained_on: Option<Fingerprint>,
    classifier: Option<Arc<Classifier>>,
    /// A training runs in the background.
    training: bool,
}

/// The classifier of the categorized operations, managed as Tauri state. Training takes from
/// a fraction of a second (release build) to many seconds (debug build), so suggestions never
/// wait for it: they use the last classifier while a new one trains in the background.
#[derive(Clone, Default)]
pub struct ClassifierCache(Arc<Mutex<CacheState>>);

impl ClassifierCache {
    /// The last trained classifier, `None` before the first training or while there are too
    /// few categorized operations. Starts a training in the background when the categorized
    /// operations changed since.
    pub async fn get(&self, pool: &SqlitePool) -> AppResult<Option<Arc<Classifier>>> {
        let fingerprint = fingerprint(pool).await?;
        let mut state = self.0.lock().expect("classifier lock");
        if state.trained_on.as_ref() != Some(&fingerprint) && !state.training {
            state.training = true;
            let (cache, pool) = (self.clone(), pool.clone());
            tauri::async_runtime::spawn(async move {
                // A failed training is tried again on the next suggestion.
                let _ = cache.train(&pool).await;
            });
        }
        Ok(state.classifier.clone())
    }

    /// Trains the classifier on the current categorized operations, and waits for it.
    pub async fn refresh(&self, pool: &SqlitePool) -> AppResult<Option<Arc<Classifier>>> {
        self.0.lock().expect("classifier lock").training = true;
        self.train(pool).await
    }

    async fn train(&self, pool: &SqlitePool) -> AppResult<Option<Arc<Classifier>>> {
        let trained = async {
            let fingerprint = fingerprint(pool).await?;
            let examples = examples(pool).await?;
            let classifier =
                tauri::async_runtime::spawn_blocking(move || Classifier::train(&examples))
                    .await
                    .ok()
                    .flatten()
                    .map(Arc::new);
            Ok::<_, crate::error::AppError>((fingerprint, classifier))
        }
        .await;
        let mut state = self.0.lock().expect("classifier lock");
        state.training = false;
        let (fingerprint, classifier) = trained?;
        state.trained_on = Some(fingerprint);
        state.classifier = classifier.clone();
        Ok(classifier)
    }
}

#[cfg(test)]
mod tests;
