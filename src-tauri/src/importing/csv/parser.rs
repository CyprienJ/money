use chrono::NaiveDate;

use super::amount::parse_cents;
use super::{AmountColumns, CsvError, CsvMapping};
use crate::importing::RawLine;

/// Reads the statement lines of a CSV file with the given mapping. Blank lines are ignored;
/// any other line that cannot be read fails the whole file, with its line number.
pub fn parse(bytes: &[u8], mapping: &CsvMapping) -> Result<Vec<RawLine>, CsvError> {
    let delimiter = mapping.validate()?;

    let encoding = mapping.encoding.encoding();
    let (text, malformed) = encoding.decode_with_bom_removal(bytes);
    if malformed {
        return Err(CsvError::InvalidEncoding(encoding.name()));
    }
    let data = skip_lines(&text, mapping.skip_lines as usize);

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut lines = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|error| CsvError::Malformed {
            line: file_line(data, mapping.skip_lines, error.position()),
            message: error.to_string(),
        })?;
        if record.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        let line = file_line(data, mapping.skip_lines, record.position());
        lines.push(read_line(&record, line, mapping)?);
    }
    Ok(lines)
}

/// One-based line number in the whole file. Computed from the byte offset, because the line
/// count of the `csv` crate ignores blank lines. A record position may point at the blank
/// lines before the record, so the line terminators there are counted too.
fn file_line(data: &str, skipped: u32, position: Option<&csv::Position>) -> u64 {
    let bytes = data.as_bytes();
    let mut offset = position
        .map_or(0, |position| position.byte() as usize)
        .min(bytes.len());
    while matches!(bytes.get(offset), Some(b'\r' | b'\n')) {
        offset += 1;
    }
    let newlines = bytes[..offset]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count();
    u64::from(skipped) + newlines as u64 + 1
}

fn skip_lines(text: &str, count: usize) -> &str {
    if count == 0 {
        return text;
    }
    text.match_indices('\n')
        .nth(count - 1)
        .map_or("", |(index, _)| &text[index + 1..])
}

fn read_line(
    record: &csv::StringRecord,
    line: u64,
    mapping: &CsvMapping,
) -> Result<RawLine, CsvError> {
    let field = |column: u32| {
        record
            .get(column as usize)
            .map(str::trim)
            .ok_or(CsvError::MissingColumn { line, column })
    };
    let amount = |column: u32| {
        let value = field(column)?;
        parse_cents(value, mapping.decimal_separator).map_err(|()| CsvError::InvalidAmount {
            line,
            value: value.to_owned(),
        })
    };

    let date_text = field(mapping.column_date)?;
    let date = NaiveDate::parse_from_str(date_text, &mapping.date_format).map_err(|_| {
        CsvError::InvalidDate {
            line,
            value: date_text.to_owned(),
        }
    })?;

    let label = field(mapping.column_label)?;
    if label.is_empty() {
        return Err(CsvError::EmptyLabel { line });
    }

    let amount_cents = match mapping.amount_columns {
        AmountColumns::Signed { amount: column } => {
            amount(column)?.ok_or(CsvError::MissingAmount { line })?
        }
        AmountColumns::DebitCredit { debit, credit } => {
            let debit = amount(debit)?.filter(|&cents| cents != 0);
            let credit = amount(credit)?.filter(|&cents| cents != 0);
            match (debit, credit) {
                (Some(debit), None) => -debit.abs(),
                (None, Some(credit)) => credit.abs(),
                (Some(_), Some(_)) => return Err(CsvError::BothDebitAndCredit { line }),
                (None, None) => return Err(CsvError::MissingAmount { line }),
            }
        }
    };
    if amount_cents == 0 {
        return Err(CsvError::ZeroAmount { line });
    }

    Ok(RawLine {
        date,
        value_date: None,
        label: label.to_owned(),
        amount_cents,
    })
}
