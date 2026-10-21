//! Automatic detection of the CSV format of a bank export.
//!
//! The start of the file is analysed step by step: encoding, delimiter, first data row (and
//! so the lines to skip, header included), date column and format, decimal separator, amount
//! columns and label column. Column titles are used first ("Date", "Libellé", "Montant",
//! "Débit", "Crédit"…), the content otherwise. A proposal is kept only when the parser reads
//! the whole file with it.

use super::amount::parse_cents;
use super::parser::parse_date;
use super::{AmountColumns, CsvEncoding, CsvMapping, DecimalSeparator, StateFilter, parse};

/// Lines analysed at the start of the file.
const SAMPLE_LINES: usize = 200;

const DELIMITERS: [char; 4] = [';', ',', '\t', '|'];

/// Date formats tried, most specific first.
const DATE_FORMATS: [&str; 5] = ["%d/%m/%Y", "%Y-%m-%d", "%d-%m-%Y", "%d.%m.%Y", "%d/%m/%y"];

/// Column titles, lowercased and without accents, matched as substrings.
const DATE_TITLES: [&str; 2] = ["date", "jour"];
const LABEL_TITLES: [&str; 6] = [
    "libelle",
    "label",
    "description",
    "operation",
    "detail",
    "intitule",
];
const AMOUNT_TITLES: [&str; 3] = ["montant", "amount", "somme"];
const DEBIT_TITLES: [&str; 2] = ["debit", "sortie"];
const CREDIT_TITLES: [&str; 2] = ["credit", "entree"];
/// Numeric columns that are not the operation amount.
const OTHER_NUMBER_TITLES: [&str; 4] = ["solde", "balance", "fee", "frais"];
const FEE_TITLES: [&str; 2] = ["fee", "frais"];
const STATE_TITLES: [&str; 3] = ["state", "etat", "statut"];

/// The mapping that reads the file, or `None` when no consistent format is found.
pub fn detect(bytes: &[u8]) -> Option<CsvMapping> {
    let encoding = detect_encoding(bytes);
    let (text, _) = encoding.encoding().decode_with_bom_removal(bytes);
    let lines: Vec<&str> = text
        .split('\n')
        .take(SAMPLE_LINES)
        .map(|line| line.trim_end_matches('\r'))
        .collect();

    let delimiter = detect_delimiter(&lines)?;
    let rows: Vec<Vec<String>> = lines.iter().map(|line| split(line, delimiter)).collect();
    let width = data_width(&rows)?;

    let (first_data, date_column, date_format) = detect_dates(&rows, width)?;
    let header = first_data
        .checked_sub(1)
        .map(|index| &rows[index])
        .filter(|row| row.len() == width)
        .map(|row| row.iter().map(|title| normalize(title)).collect::<Vec<_>>());
    let data: Vec<&Vec<String>> = rows[first_data..]
        .iter()
        .filter(|row| row.len() == width)
        .collect();

    let decimal_separator = detect_decimal_separator(&data, date_column);
    let numeric: Vec<usize> = (0..width)
        .filter(|&column| column != date_column && is_numeric(&data, column, decimal_separator))
        .collect();
    let amount_columns = detect_amounts(&data, header.as_deref(), &numeric, decimal_separator)?;
    let mut used: Vec<usize> = match amount_columns {
        AmountColumns::Signed { amount } => vec![amount as usize],
        AmountColumns::DebitCredit { debit, credit } => vec![debit as usize, credit as usize],
    };
    let column_fee = numeric
        .iter()
        .copied()
        .find(|&c| !used.contains(&c) && has_title(header.as_deref(), c, &FEE_TITLES));
    let state_filter = detect_state(&data, header.as_deref(), width, date_column, &numeric);
    used.extend(column_fee);
    used.extend(state_filter.as_ref().map(|filter| filter.column as usize));
    let column_label = detect_label(
        &data,
        header.as_deref(),
        width,
        date_column,
        &numeric,
        &used,
    )?;

    let mapping = CsvMapping {
        delimiter,
        encoding,
        date_format: date_format.to_owned(),
        decimal_separator,
        column_date: date_column as u32,
        column_label: column_label as u32,
        amount_columns,
        skip_lines: first_data as u32,
        column_fee: column_fee.map(|c| c as u32),
        state_filter,
    };
    // Kept only when it reads the whole file.
    let lines = parse(bytes, &mapping).ok()?;
    (!lines.is_empty()).then_some(mapping)
}

fn detect_encoding(bytes: &[u8]) -> CsvEncoding {
    let without_bom = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if std::str::from_utf8(without_bom).is_ok() {
        CsvEncoding::Utf8
    } else {
        CsvEncoding::Windows1252
    }
}

/// Fields of a line, honouring double quotes.
fn split(line: &str, delimiter: char) -> Vec<String> {
    if line.trim().is_empty() {
        return Vec::new();
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter as u8)
        .from_reader(line.as_bytes());
    reader
        .records()
        .next()
        .and_then(Result::ok)
        .map(|record| record.iter().map(|field| field.trim().to_owned()).collect())
        .unwrap_or_default()
}

/// The delimiter splitting the most lines into the same number of fields (at least 3).
fn detect_delimiter(lines: &[&str]) -> Option<char> {
    DELIMITERS
        .iter()
        .filter_map(|&delimiter| {
            let rows: Vec<Vec<String>> = lines.iter().map(|line| split(line, delimiter)).collect();
            let width = data_width(&rows)?;
            let count = rows.iter().filter(|row| row.len() == width).count();
            Some((count, delimiter))
        })
        // Most consistent lines; on a tie, the order of `DELIMITERS`.
        .fold(None, |best: Option<(usize, char)>, candidate| match best {
            Some(best) if best.0 >= candidate.0 => Some(best),
            _ => Some(candidate),
        })
        .map(|(_, delimiter)| delimiter)
}

/// Most frequent number of fields among lines with at least 3 fields.
fn data_width(rows: &[Vec<String>]) -> Option<usize> {
    let mut counts = std::collections::BTreeMap::new();
    for row in rows.iter().filter(|row| row.len() >= 3) {
        *counts.entry(row.len()).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(width, count)| (count, width))
        .map(|(width, _)| width)
}

/// First data row, date column and format: the (column, format) reading the most rows, the
/// first data row being the first of them.
fn detect_dates(rows: &[Vec<String>], width: usize) -> Option<(usize, usize, &'static str)> {
    let mut best: Option<(usize, usize, &'static str)> = None;
    let mut best_count = 0;
    for column in 0..width {
        for format in DATE_FORMATS {
            let matches: Vec<usize> = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row.len() == width)
                .filter(|(_, row)| parse_date(&row[column], format).is_some())
                .map(|(index, _)| index)
                .collect();
            if matches.len() > best_count {
                best_count = matches.len();
                best = Some((matches[0], column, format));
            }
        }
    }
    best
}

/// Lowercased title without accents, to match column titles.
fn normalize(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'û' | 'ù' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn has_title(header: Option<&[String]>, column: usize, titles: &[&str]) -> bool {
    header.is_some_and(|header| titles.iter().any(|title| header[column].contains(title)))
}

/// The separator of the amounts: the last `,` or `.` of a value, preceded by a digit and
/// followed by one or two digits only, counted over the whole sample.
fn detect_decimal_separator(data: &[&Vec<String>], date_column: usize) -> DecimalSeparator {
    let (mut commas, mut dots) = (0, 0);
    for row in data {
        for (column, field) in row.iter().enumerate() {
            if column == date_column {
                continue;
            }
            match decimal_mark(field.trim_end_matches(['€', ' '])) {
                Some(',') => commas += 1,
                Some('.') => dots += 1,
                _ => {}
            }
        }
    }
    if dots > commas {
        DecimalSeparator::Dot
    } else {
        DecimalSeparator::Comma
    }
}

fn decimal_mark(value: &str) -> Option<char> {
    let position = value.rfind([',', '.'])?;
    let (before, after) = (&value[..position], &value[position + 1..]);
    let digits_after = (1..=2).contains(&after.len()) && after.bytes().all(|b| b.is_ascii_digit());
    let digit_before = before.bytes().last().is_some_and(|b| b.is_ascii_digit());
    (digits_after && digit_before).then(|| value[position..].chars().next().expect("mark found"))
}

/// A column whose filled cells are all amounts, with at least one filled cell.
fn is_numeric(data: &[&Vec<String>], column: usize, separator: DecimalSeparator) -> bool {
    let values: Vec<&str> = data
        .iter()
        .map(|row| row[column].as_str())
        .filter(|value| !value.is_empty())
        .collect();
    !values.is_empty()
        && values
            .iter()
            .all(|value| parse_cents(value, separator).is_ok_and(|cents| cents.is_some()))
}

fn amount(value: &str, separator: DecimalSeparator) -> Option<i64> {
    parse_cents(value, separator).ok().flatten()
}

fn detect_amounts(
    data: &[&Vec<String>],
    header: Option<&[String]>,
    numeric: &[usize],
    separator: DecimalSeparator,
) -> Option<AmountColumns> {
    let titled = |titles: &[&str]| {
        numeric
            .iter()
            .copied()
            .find(|&c| has_title(header, c, titles))
    };
    if let (Some(debit), Some(credit)) = (titled(&DEBIT_TITLES), titled(&CREDIT_TITLES)) {
        return Some(AmountColumns::DebitCredit {
            debit: debit as u32,
            credit: credit as u32,
        });
    }
    if let Some(amount) = titled(&AMOUNT_TITLES) {
        return Some(AmountColumns::Signed {
            amount: amount as u32,
        });
    }

    // Without titles: the content decides, balances and fees aside.
    let candidates: Vec<usize> = numeric
        .iter()
        .copied()
        .filter(|&c| !has_title(header, c, &OTHER_NUMBER_TITLES))
        .collect();
    // Two columns never filled together, one of them on each row: debit and credit.
    for (index, &first) in candidates.iter().enumerate() {
        for &second in &candidates[index + 1..] {
            let exclusive = data.iter().all(|row| {
                let filled = |c: usize| amount(&row[c], separator).is_some_and(|cents| cents != 0);
                filled(first) != filled(second) || (!filled(first) && !filled(second))
            });
            let both_used = [first, second].iter().all(|&c| {
                data.iter()
                    .any(|row| amount(&row[c], separator).is_some_and(|v| v != 0))
            });
            if exclusive && both_used {
                return Some(AmountColumns::DebitCredit {
                    debit: first as u32,
                    credit: second as u32,
                });
            }
        }
    }
    // Otherwise a signed amount: the first column with negative values, or the first one.
    let signed = candidates
        .iter()
        .copied()
        .find(|&c| {
            data.iter()
                .any(|row| amount(&row[c], separator).is_some_and(|v| v < 0))
        })
        .or_else(|| candidates.first().copied())?;
    Some(AmountColumns::Signed {
        amount: signed as u32,
    })
}

/// A state column, found by its title, keeping its most frequent value (`COMPLETED`).
fn detect_state(
    data: &[&Vec<String>],
    header: Option<&[String]>,
    width: usize,
    date_column: usize,
    numeric: &[usize],
) -> Option<StateFilter> {
    let column = (0..width).find(|&c| {
        c != date_column && !numeric.contains(&c) && has_title(header, c, &STATE_TITLES)
    })?;
    let mut counts = std::collections::BTreeMap::new();
    for row in data.iter().filter(|row| !row[column].is_empty()) {
        *counts.entry(row[column].as_str()).or_insert(0usize) += 1;
    }
    let (keep, _) = counts.into_iter().max_by_key(|&(_, count)| count)?;
    Some(StateFilter {
        column: column as u32,
        keep: keep.to_owned(),
    })
}

/// The label column: by title, otherwise the text column with the longest values.
fn detect_label(
    data: &[&Vec<String>],
    header: Option<&[String]>,
    width: usize,
    date_column: usize,
    numeric: &[usize],
    used: &[usize],
) -> Option<usize> {
    let free = |c: &usize| *c != date_column && !numeric.contains(c) && !used.contains(c);
    let columns: Vec<usize> = (0..width).filter(free).collect();
    if let Some(&titled) = columns
        .iter()
        .find(|&&c| has_title(header, c, &LABEL_TITLES))
    {
        return Some(titled);
    }
    columns
        .into_iter()
        .filter(|&c| !has_title(header, c, &DATE_TITLES))
        .max_by_key(|&c| data.iter().map(|row| row[c].chars().count()).sum::<usize>())
}

#[cfg(test)]
mod tests;
