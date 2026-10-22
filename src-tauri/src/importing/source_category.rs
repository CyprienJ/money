//! Categories given by a QIF file ("Alimentation:Courses"), and the categories of the
//! application chosen for them, remembered for the next imports. Without a remembered choice,
//! a category of the file goes to the category of the same name: "Parent:Enfant" to the
//! subcategory of that path, or else the last part to the only category of that name, ignoring
//! case and accents. A transfer `[Compte]` goes to the internal transfer category.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqliteConnection;

use crate::error::AppResult;
use crate::labeling::rules::fold;
use crate::operation::category::Category;

/// A category of the file, in the import preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceCategory {
    pub name: String,
    /// Category of the application its lines get; `None`: categorized by the rules and the
    /// memory, as a CSV line.
    pub category_id: Option<i64>,
    /// The category comes from an earlier choice, not from the name.
    pub remembered: bool,
    pub line_count: u32,
}

/// Category chosen in the preview for a category of the file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceCategoryChoice {
    pub name: String,
    /// `None` forgets the earlier choice.
    pub category_id: Option<i64>,
}

pub fn is_transfer(name: &str) -> bool {
    name.starts_with('[') && name.ends_with(']')
}

/// Remembered choices, by category of the file.
pub async fn remembered(conn: &mut SqliteConnection) -> AppResult<HashMap<String, i64>> {
    let rows = sqlx::query!(
        r#"SELECT source_category AS "source_category!", category_id FROM source_category_mapping"#
    )
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.source_category, row.category_id))
        .collect())
}

pub async fn remember(
    conn: &mut SqliteConnection,
    choices: &[SourceCategoryChoice],
) -> AppResult<()> {
    for choice in choices {
        match choice.category_id {
            Some(category_id) => {
                sqlx::query!(
                    "INSERT INTO source_category_mapping (source_category, category_id)
                     VALUES (?1, ?2)
                     ON CONFLICT (source_category) DO UPDATE SET category_id = excluded.category_id",
                    choice.name,
                    category_id
                )
                .execute(&mut *conn)
                .await?;
            }
            None => {
                sqlx::query!(
                    "DELETE FROM source_category_mapping WHERE source_category = ?1",
                    choice.name
                )
                .execute(&mut *conn)
                .await?;
            }
        }
    }
    Ok(())
}

/// The category of the application of the same name as a category of the file.
pub fn match_by_name(name: &str, categories: &[Category]) -> Option<i64> {
    let path = |category: &Category| -> String {
        let parent = category
            .parent_id
            .and_then(|id| categories.iter().find(|c| c.id == id));
        match parent {
            Some(parent) => format!("{}:{}", fold(&parent.name), fold(&category.name)),
            None => fold(&category.name),
        }
    };
    let wanted: String = name.split(':').map(fold).collect::<Vec<_>>().join(":");
    if let Some(category) = categories.iter().find(|c| path(c) == wanted) {
        return Some(category.id);
    }
    let last = fold(name.rsplit(':').next().unwrap_or(name));
    let mut same_name = categories.iter().filter(|c| fold(&c.name) == last);
    match (same_name.next(), same_name.next()) {
        (Some(category), None) => Some(category.id),
        _ => None,
    }
}
