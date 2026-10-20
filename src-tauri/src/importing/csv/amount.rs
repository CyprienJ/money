//! Amount parsing into cents, without floating point.

use super::DecimalSeparator;

/// Spaces banks put between thousands groups: regular, no-break and narrow no-break.
const SPACES: [char; 3] = [' ', '\u{00A0}', '\u{202F}'];

/// Parses an amount such as `-1 234,56`, `1.234,56 €` or `+12.5` into cents.
/// Returns `Ok(None)` for an empty cell and `Err(())` for anything that is not an amount.
pub(super) fn parse_cents(text: &str, separator: DecimalSeparator) -> Result<Option<i64>, ()> {
    let mut text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    for currency in ["€", "EUR"] {
        if let Some(rest) = text.strip_suffix(currency) {
            text = rest.trim_end();
        }
    }

    let (negative, unsigned) = split_sign(text);
    let (integer, fraction) = match unsigned.split_once(separator.decimal()) {
        Some((integer, fraction)) => (integer, fraction),
        None => (unsigned, ""),
    };

    let units = parse_integer(integer, separator.thousands())?;
    if fraction.len() > 2 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    if integer.is_empty() && fraction.is_empty() {
        return Err(());
    }
    let cents = format!("{fraction:0<2}").parse::<i64>().map_err(|_| ())?;

    let total = units
        .checked_mul(100)
        .and_then(|value| value.checked_add(cents))
        .ok_or(())?;
    Ok(Some(if negative { -total } else { total }))
}

/// Strips a leading `-`, `−` or `+`, or a trailing `-`.
fn split_sign(text: &str) -> (bool, &str) {
    if let Some(rest) = text.strip_prefix(['-', '\u{2212}']) {
        (true, rest.trim_start())
    } else if let Some(rest) = text.strip_prefix('+') {
        (false, rest.trim_start())
    } else if let Some(rest) = text.strip_suffix('-') {
        (true, rest.trim_end())
    } else {
        (false, text)
    }
}

/// Parses the integer part, where thousands groups may be separated by spaces or by
/// `thousands`. Groups after the first must have exactly three digits, so that `12.50` read
/// with a comma decimal separator is rejected instead of becoming 1 250.
fn parse_integer(text: &str, thousands: char) -> Result<i64, ()> {
    if text.is_empty() {
        return Ok(0);
    }
    let groups: Vec<&str> = text
        .split(|character| character == thousands || SPACES.contains(&character))
        .collect();
    let (first, rest) = groups.split_first().ok_or(())?;
    let valid_digits = |group: &str| !group.is_empty() && group.bytes().all(|b| b.is_ascii_digit());
    if !valid_digits(first) || (!rest.is_empty() && first.len() > 3) {
        return Err(());
    }
    if rest
        .iter()
        .any(|group| group.len() != 3 || !valid_digits(group))
    {
        return Err(());
    }
    groups.concat().parse::<i64>().map_err(|_| ())
}
