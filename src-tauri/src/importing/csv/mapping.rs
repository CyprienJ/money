//! Saved CSV mappings, named by bank or format.

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::{AmountColumns, CsvEncoding, CsvMapping, DecimalSeparator, StateFilter, parse};
use crate::error::{AppResult, ErrorKind};

/// A mapping saved under a name, e.g. "Banque X - export CSV".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedCsvMapping {
    pub id: i64,
    pub name: String,
    pub mapping: CsvMapping,
}

/// A mapping to save, on creation or update.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewCsvMapping {
    pub name: String,
    pub mapping: CsvMapping,
}

/// A saved mapping that reads a file without error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MappingSuggestion {
    pub mapping: SavedCsvMapping,
    /// Statement lines read from the file with this mapping.
    pub line_count: u32,
}

/// Serialized for the frontend as `{ code, params }`, to show a message in French.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(
    tag = "code",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CsvMappingError {
    #[error("CSV mapping {0} not found")]
    NotFound(i64),

    #[error("CSV mapping name cannot be empty")]
    EmptyName,

    #[error("a CSV mapping named {0:?} already exists")]
    DuplicateName(String),

    #[error("CSV mapping {0} has invalid stored settings")]
    Corrupt(i64),
}

impl CsvMappingError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            CsvMappingError::NotFound(_) => ErrorKind::NotFound,
            CsvMappingError::Corrupt(_) => ErrorKind::Database,
            _ => ErrorKind::Validation,
        }
    }
}

/// `csv_mapping` row; columns are zero-based indexes.
struct Row {
    id: i64,
    name: String,
    delimiter: String,
    encoding: String,
    date_format: String,
    decimal_separator: String,
    column_date: i64,
    column_label: i64,
    column_amount: Option<i64>,
    column_debit: Option<i64>,
    column_credit: Option<i64>,
    skip_lines: i64,
    column_fee: Option<i64>,
    column_state: Option<i64>,
    state_keep: Option<String>,
}

impl Row {
    fn into_saved(self) -> Result<SavedCsvMapping, CsvMappingError> {
        let corrupt = CsvMappingError::Corrupt(self.id);
        let column =
            |value: i64| u32::try_from(value).map_err(|_| CsvMappingError::Corrupt(self.id));
        let mut delimiter = self.delimiter.chars();
        let (Some(delimiter), None) = (delimiter.next(), delimiter.next()) else {
            return Err(corrupt);
        };
        let amount_columns = match (self.column_amount, self.column_debit, self.column_credit) {
            (Some(amount), _, _) => AmountColumns::Signed {
                amount: column(amount)?,
            },
            (None, Some(debit), Some(credit)) => AmountColumns::DebitCredit {
                debit: column(debit)?,
                credit: column(credit)?,
            },
            _ => return Err(corrupt),
        };
        let mapping = CsvMapping {
            delimiter,
            encoding: encoding_from_db(&self.encoding).ok_or(CsvMappingError::Corrupt(self.id))?,
            date_format: self.date_format,
            decimal_separator: decimal_from_db(&self.decimal_separator)
                .ok_or(CsvMappingError::Corrupt(self.id))?,
            column_date: column(self.column_date)?,
            column_label: column(self.column_label)?,
            amount_columns,
            skip_lines: column(self.skip_lines)?,
            column_fee: self.column_fee.map(column).transpose()?,
            state_filter: match (self.column_state, self.state_keep) {
                (Some(state), Some(keep)) => Some(StateFilter {
                    column: column(state)?,
                    keep,
                }),
                (None, None) => None,
                _ => return Err(CsvMappingError::Corrupt(self.id)),
            },
        };
        Ok(SavedCsvMapping {
            id: self.id,
            name: self.name,
            mapping,
        })
    }
}

fn encoding_to_db(encoding: CsvEncoding) -> &'static str {
    match encoding {
        CsvEncoding::Utf8 => "UTF-8",
        CsvEncoding::Iso8859_1 => "ISO-8859-1",
        CsvEncoding::Windows1252 => "WINDOWS-1252",
    }
}

fn encoding_from_db(text: &str) -> Option<CsvEncoding> {
    [
        CsvEncoding::Utf8,
        CsvEncoding::Iso8859_1,
        CsvEncoding::Windows1252,
    ]
    .into_iter()
    .find(|&encoding| encoding_to_db(encoding) == text)
}

fn decimal_to_db(separator: DecimalSeparator) -> &'static str {
    match separator {
        DecimalSeparator::Comma => ",",
        DecimalSeparator::Dot => ".",
    }
}

fn decimal_from_db(text: &str) -> Option<DecimalSeparator> {
    [DecimalSeparator::Comma, DecimalSeparator::Dot]
        .into_iter()
        .find(|&separator| decimal_to_db(separator) == text)
}

/// Lists saved mappings by name.
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<SavedCsvMapping>> {
    let rows = sqlx::query_as!(
        Row,
        r#"SELECT id AS "id!", name, delimiter, encoding, date_format, decimal_separator,
                  column_date, column_label, column_amount, column_debit, column_credit, skip_lines,
                  column_fee, column_state, state_keep
           FROM csv_mapping
           ORDER BY name COLLATE NOCASE"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(Row::into_saved)
        .collect::<Result<_, _>>()?)
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<SavedCsvMapping> {
    let row = sqlx::query_as!(
        Row,
        r#"SELECT id AS "id!", name, delimiter, encoding, date_format, decimal_separator,
                  column_date, column_label, column_amount, column_debit, column_credit, skip_lines,
                  column_fee, column_state, state_keep
           FROM csv_mapping
           WHERE id = ?1"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(CsvMappingError::NotFound(id))?;
    Ok(row.into_saved()?)
}

pub async fn create(pool: &SqlitePool, new: NewCsvMapping) -> AppResult<SavedCsvMapping> {
    let values = Values::check(&new)?;
    let id = sqlx::query!(
        "INSERT INTO csv_mapping (name, delimiter, encoding, date_format, decimal_separator,
                                  column_date, column_label, column_amount, column_debit,
                                  column_credit, skip_lines, column_fee, column_state, state_keep)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        values.name,
        values.delimiter,
        values.encoding,
        new.mapping.date_format,
        values.decimal_separator,
        new.mapping.column_date,
        new.mapping.column_label,
        values.column_amount,
        values.column_debit,
        values.column_credit,
        new.mapping.skip_lines,
        new.mapping.column_fee,
        values.column_state,
        values.state_keep
    )
    .execute(pool)
    .await
    .map_err(|error| duplicate_name(error, &values.name))?
    .last_insert_rowid();
    get(pool, id).await
}

pub async fn update(
    pool: &SqlitePool,
    id: i64,
    changes: NewCsvMapping,
) -> AppResult<SavedCsvMapping> {
    let values = Values::check(&changes)?;
    let updated = sqlx::query!(
        "UPDATE csv_mapping
         SET name = ?2, delimiter = ?3, encoding = ?4, date_format = ?5, decimal_separator = ?6,
             column_date = ?7, column_label = ?8, column_amount = ?9, column_debit = ?10,
             column_credit = ?11, skip_lines = ?12, column_fee = ?13, column_state = ?14,
             state_keep = ?15
         WHERE id = ?1",
        id,
        values.name,
        values.delimiter,
        values.encoding,
        changes.mapping.date_format,
        values.decimal_separator,
        changes.mapping.column_date,
        changes.mapping.column_label,
        values.column_amount,
        values.column_debit,
        values.column_credit,
        changes.mapping.skip_lines,
        changes.mapping.column_fee,
        values.column_state,
        values.state_keep
    )
    .execute(pool)
    .await
    .map_err(|error| duplicate_name(error, &values.name))?
    .rows_affected();
    if updated == 0 {
        return Err(CsvMappingError::NotFound(id).into());
    }
    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let deleted = sqlx::query!("DELETE FROM csv_mapping WHERE id = ?1", id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(CsvMappingError::NotFound(id).into());
    }
    Ok(())
}

/// Saved mappings that read the whole file without error, the most lines read first.
pub async fn suggest(pool: &SqlitePool, bytes: &[u8]) -> AppResult<Vec<MappingSuggestion>> {
    let mut suggestions: Vec<MappingSuggestion> = list(pool)
        .await?
        .into_iter()
        .filter_map(|saved| {
            let lines = parse(bytes, &saved.mapping).ok()?;
            let line_count = u32::try_from(lines.len()).ok().filter(|&count| count > 0)?;
            Some(MappingSuggestion {
                mapping: saved,
                line_count,
            })
        })
        .collect();
    // Stable sort: equal counts keep the name order of `list`.
    suggestions.sort_by_key(|suggestion| std::cmp::Reverse(suggestion.line_count));
    Ok(suggestions)
}

/// Column values of a mapping to save, once checked.
struct Values {
    name: String,
    delimiter: String,
    encoding: &'static str,
    decimal_separator: &'static str,
    column_amount: Option<u32>,
    column_debit: Option<u32>,
    column_credit: Option<u32>,
    column_state: Option<u32>,
    state_keep: Option<String>,
}

impl Values {
    fn check(new: &NewCsvMapping) -> AppResult<Self> {
        let name = new.name.trim();
        if name.is_empty() {
            return Err(CsvMappingError::EmptyName.into());
        }
        new.mapping.validate()?;
        let (column_amount, column_debit, column_credit) = match new.mapping.amount_columns {
            AmountColumns::Signed { amount } => (Some(amount), None, None),
            AmountColumns::DebitCredit { debit, credit } => (None, Some(debit), Some(credit)),
        };
        Ok(Values {
            name: name.to_owned(),
            delimiter: new.mapping.delimiter.to_string(),
            encoding: encoding_to_db(new.mapping.encoding),
            decimal_separator: decimal_to_db(new.mapping.decimal_separator),
            column_amount,
            column_debit,
            column_credit,
            column_state: new
                .mapping
                .state_filter
                .as_ref()
                .map(|filter| filter.column),
            state_keep: new
                .mapping
                .state_filter
                .as_ref()
                .map(|filter| filter.keep.trim().to_owned()),
        })
    }
}

fn duplicate_name(error: sqlx::Error, name: &str) -> crate::error::AppError {
    match error.as_database_error() {
        Some(database) if database.is_unique_violation() => {
            CsvMappingError::DuplicateName(name.to_owned()).into()
        }
        _ => error.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::memory_pool;
    use crate::error::AppError;
    use crate::importing::csv::CsvError;

    fn run<F: Future<Output = ()>>(test: impl FnOnce(SqlitePool) -> F) {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            test(pool).await;
        });
    }

    fn debit_credit(skip_lines: u32) -> CsvMapping {
        CsvMapping {
            delimiter: ';',
            encoding: CsvEncoding::Windows1252,
            date_format: "%d/%m/%Y".to_owned(),
            decimal_separator: DecimalSeparator::Comma,
            column_date: 0,
            column_label: 1,
            amount_columns: AmountColumns::DebitCredit {
                debit: 2,
                credit: 3,
            },
            skip_lines,
            column_fee: None,
            state_filter: None,
        }
    }

    fn signed() -> CsvMapping {
        CsvMapping {
            delimiter: ',',
            encoding: CsvEncoding::Utf8,
            date_format: "%Y-%m-%d".to_owned(),
            decimal_separator: DecimalSeparator::Dot,
            column_date: 0,
            column_label: 1,
            amount_columns: AmountColumns::Signed { amount: 2 },
            skip_lines: 1,
            column_fee: None,
            state_filter: None,
        }
    }

    fn named(name: &str, mapping: CsvMapping) -> NewCsvMapping {
        NewCsvMapping {
            name: name.to_owned(),
            mapping,
        }
    }

    #[test]
    fn round_trips_both_amount_layouts() {
        run(|pool| async move {
            let a = create(&pool, named("  Banque A  ", debit_credit(5)))
                .await
                .unwrap();
            let b = create(&pool, named("Banque B", signed())).await.unwrap();
            assert_eq!(a.name, "Banque A");
            assert_eq!(a.mapping, debit_credit(5));
            assert_eq!(b.mapping, signed());
            assert_eq!(list(&pool).await.unwrap(), vec![a, b]);
        });
    }

    #[test]
    fn round_trips_fees_and_state() {
        run(|pool| async move {
            let mapping = CsvMapping {
                column_fee: Some(3),
                state_filter: Some(StateFilter {
                    column: 4,
                    keep: " COMPLETED ".to_owned(),
                }),
                ..signed()
            };
            let saved = create(&pool, named("Revolut", mapping.clone()))
                .await
                .unwrap();
            // The value to keep is stored trimmed.
            assert_eq!(
                saved.mapping.state_filter,
                Some(StateFilter {
                    column: 4,
                    keep: "COMPLETED".to_owned(),
                })
            );
            assert_eq!(saved.mapping.column_fee, Some(3));
            assert_eq!(get(&pool, saved.id).await.unwrap(), saved);

            // Back to no fees and every state.
            let updated = update(&pool, saved.id, named("Revolut", signed()))
                .await
                .unwrap();
            assert_eq!(updated.mapping, signed());
            assert!(matches!(
                create(
                    &pool,
                    named(
                        "Vide",
                        CsvMapping {
                            state_filter: Some(StateFilter {
                                column: 4,
                                keep: String::new(),
                            }),
                            ..signed()
                        }
                    )
                )
                .await,
                Err(AppError::Csv(CsvError::EmptyStateValue))
            ));
        });
    }

    #[test]
    fn updates_and_deletes() {
        run(|pool| async move {
            let saved = create(&pool, named("A", debit_credit(0))).await.unwrap();
            let updated = update(&pool, saved.id, named("B", signed())).await.unwrap();
            assert_eq!((updated.name.as_str(), &updated.mapping), ("B", &signed()));

            delete(&pool, saved.id).await.unwrap();
            assert!(matches!(
                get(&pool, saved.id).await,
                Err(AppError::CsvMapping(CsvMappingError::NotFound(_)))
            ));
            assert!(matches!(
                delete(&pool, saved.id).await,
                Err(AppError::CsvMapping(CsvMappingError::NotFound(_)))
            ));
            assert!(matches!(
                update(&pool, saved.id, named("C", signed())).await,
                Err(AppError::CsvMapping(CsvMappingError::NotFound(_)))
            ));
        });
    }

    #[test]
    fn rejects_empty_and_duplicate_names() {
        run(|pool| async move {
            assert!(matches!(
                create(&pool, named(" ", signed())).await,
                Err(AppError::CsvMapping(CsvMappingError::EmptyName))
            ));
            create(&pool, named("A", signed())).await.unwrap();
            let other = create(&pool, named("B", signed())).await.unwrap();
            assert!(matches!(
                create(&pool, named("A ", signed())).await,
                Err(AppError::CsvMapping(CsvMappingError::DuplicateName(name))) if name == "A"
            ));
            assert!(matches!(
                update(&pool, other.id, named("A", signed())).await,
                Err(AppError::CsvMapping(CsvMappingError::DuplicateName(_)))
            ));
        });
    }

    #[test]
    fn rejects_invalid_settings() {
        run(|pool| async move {
            let mapping = CsvMapping {
                date_format: "%d/%Q".to_owned(),
                ..signed()
            };
            assert!(matches!(
                create(&pool, named("A", mapping)).await,
                Err(AppError::Csv(CsvError::InvalidDateFormat(_)))
            ));
            let mapping = CsvMapping {
                delimiter: '§',
                ..signed()
            };
            assert!(matches!(
                create(&pool, named("A", mapping)).await,
                Err(AppError::Csv(CsvError::InvalidDelimiter('§')))
            ));
        });
    }

    #[test]
    fn suggests_mappings_that_read_the_file() {
        run(|pool| async move {
            let file = "Date;Libellé;Débit;Crédit\n02/09/2026;A;1,00;\n03/09/2026;B;;2,00\n";
            // Reads both lines.
            let header = create(&pool, named("Avec en-tête", debit_credit(1)))
                .await
                .unwrap();
            // Reads the header as an operation: invalid date.
            create(&pool, named("Sans en-tête", debit_credit(0)))
                .await
                .unwrap();
            // Skips the first operation too: reads one line.
            let skip = create(&pool, named("Deux lignes", debit_credit(2)))
                .await
                .unwrap();
            create(&pool, named("Autre format", signed()))
                .await
                .unwrap();

            let suggestions = suggest(&pool, file.as_bytes()).await.unwrap();
            let found: Vec<(i64, u32)> = suggestions
                .iter()
                .map(|suggestion| (suggestion.mapping.id, suggestion.line_count))
                .collect();
            assert_eq!(found, vec![(header.id, 2), (skip.id, 1)]);
        });
    }
}
