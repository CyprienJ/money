//! Normalized labels: the key used for categorization and payees.
//!
//! The raw bank label is decoded of HTML entities and uppercased, then cleaned of what changes
//! from one operation to the next for the same payee: bank prefixes (`CB`, `PRLV SEPA`, `VIR`…),
//! dates, card numbers, IBANs and references. `raw_label` itself is never modified.

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;
use sqlx::SqlitePool;

use crate::error::AppResult;

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("normalization patterns are valid")
}

/// Month names and abbreviations, English and French, uppercased.
const MONTHS: &str = "JAN(?:UARY|V|VIER)?|FEB(?:RUARY)?|F[EÉ]V(?:R|RIER)?|MAR(?:CH|S)?|APR(?:IL)?|AVR(?:IL)?\
                      |MAY|MAI|JUNE?|JUIN|JUL(?:Y)?|JUIL(?:LET)?|AUG(?:UST)?|AO[UÛ]T\
                      |SEPT?(?:EMBER|EMBRE)?|OCT(?:OBER|OBRE)?|NOV(?:EMBER|EMBRE)?|D[EÉ]C(?:EMBER|EMBRE)?";

/// Noise removed anywhere in the label, in this order.
static NOISE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let months = MONTHS;
    // Dates in words, optionally after "LE", "DU" or "ON": "APR 5, 2025", "5 AVR. 2025",
    // "1ER AVRIL 2025", and a month with its year ("AVRIL 2025").
    let dates_in_words = [
        format!(
            r"(?:\b(?:LE|DU|ON)\s+)?\b(?:{months})\.?\s+\d{{1,2}}(?:ST|ND|RD|TH)?,?\s+\d{{4}}\b"
        ),
        format!(
            r"(?:\b(?:LE|DU|ON)\s+)?\b\d{{1,2}}(?:ER|ST|ND|RD|TH)?\s+(?:{months})\.?,?\s+\d{{4}}\b"
        ),
        format!(r"(?:\b(?:EN|DE|FOR)\s+)?\b(?:{months})\.?\s+\d{{4}}\b"),
    ];
    dates_in_words
        .into_iter()
        .chain(
            [
                // IBAN, compact or by groups of four.
                r"\b[A-Z]{2}\d{2}[A-Z0-9]{10,30}\b",
                r"\b[A-Z]{2}\d{2}(?: [A-Z0-9]{4}){3,7}(?: [A-Z0-9]{1,3})?\b",
                // Card numbers: "4417****1234", "CARTE 4417", "CARTE X1234", "X1234".
                r"\b\d{4,6}[*X]{4,}\d{4}\b",
                r"\bCARTE\s+X*\d{4}\b",
                r"\bX+\d{4}\b",
                // Due dates and references with their value: "ECH 10/26", "REF: AB12", "RUM X".
                r"\bECH(?:EANCE)?\s*:?\s*\d{1,2}[/.]\d{2,4}\b",
                r"\b(?:REF|RUM|ID EMETTEUR)\b\s*[:.]?\s*\S+",
                // Dates: "18/10", "18/10/26", "18.10.2026", optionally after "DU".
                r"\b(?:DU\s+)?\d{1,2}[/.]\d{1,2}(?:[/.]\d{2,4})?\b",
                // Separators left inside the label.
                r"[;|*]",
                r"\(\s*\)",
            ]
            .map(str::to_owned),
        )
        .map(|pattern| regex(&pattern))
        .collect()
});

/// Numeric and usual named HTML entities, left by some exports (`D&#39;ÉPARGNE`).
static ENTITY: LazyLock<Regex> =
    LazyLock::new(|| regex(r"&(?:#(\d{1,7})|#[xX]([0-9A-Fa-f]{1,6})|(amp|quot|apos|lt|gt|nbsp));"));

fn decode_entities(text: &str) -> Cow<'_, str> {
    ENTITY.replace_all(text, |captures: &regex::Captures| {
        let code = if let Some(decimal) = captures.get(1) {
            decimal.as_str().parse().ok()
        } else if let Some(hex) = captures.get(2) {
            u32::from_str_radix(hex.as_str(), 16).ok()
        } else {
            Some(match &captures[3] {
                "amp" => '&' as u32,
                "quot" => '"' as u32,
                "apos" => '\'' as u32,
                "lt" => '<' as u32,
                "gt" => '>' as u32,
                _ => ' ' as u32,
            })
        };
        code.and_then(char::from_u32)
            .map_or_else(|| captures[0].to_owned(), String::from)
    })
}

/// Bank operation prefixes, removed (repeatedly) at the start of the label.
static PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^(?:PAIEMENT\s+(?:PAR\s+)?(?:CB|CARTE)|ACHAT\s+CB|FACTURE\s+CARTE|AVOIR(?:\s+CB)?|CB|CARTE|PRLV(?:\s+SEPA)?|PRELEVEMENT(?:\s+(?:SEPA|EUROPEEN))?|VIR(?:EMENT)?(?:\s+(?:SEPA|INST(?:ANTANE)?|PERMANENT|RECU|EMIS))*(?:\s*/\s*(?:DE|A|FRM|TO))?|REMISE\s+CHQ|CHQ|CHEQUE)(?:\s+|[:/-]+\s*|$)",
    )
});

/// Characters trimmed at both ends once the label is cleaned.
const TRIMMED: &[char] = &[' ', '-', '/', ':', '.', ',', '(', ')'];

/// Normalized label of a raw bank label, e.g. `CB CARREFOUR MKT 18/10 CARTE 4417` →
/// `CARREFOUR MKT`. Falls back to the uppercased raw label when nothing would remain.
pub fn normalize_label(raw_label: &str) -> String {
    let upper = collapse(&decode_entities(raw_label).to_uppercase());

    let mut text = upper.clone();
    for pattern in NOISE.iter() {
        text = pattern.replace_all(&text, " ").into_owned();
    }
    text = collapse(&text);
    while let Some(prefix) = PREFIX.find(&text) {
        if prefix.as_str().is_empty() {
            break;
        }
        text = text[prefix.end()..].to_owned();
    }
    let text = collapse(
        &text
            .split_whitespace()
            .filter(|token| !is_reference(token))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let text = text.trim_matches(TRIMMED);

    if text.is_empty() {
        upper
    } else {
        text.to_owned()
    }
}

/// Codes that differ for every operation: 5 digits or more, or a long token mixing letters
/// and at least 4 digits (`26267IP1RT501085988`).
fn is_reference(token: &str) -> bool {
    let digits = token.chars().filter(char::is_ascii_digit).count();
    let all_digits = digits == token.chars().count();
    (all_digits && digits >= 5) || (token.chars().count() >= 8 && digits >= 4)
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Recomputes the normalized label of every operation from its raw label, so that a change of
/// the rules applies to existing data. Only rows that change are written; returns their count.
/// The category remembered for an old label carries over to the new one, the most recent winning.
pub async fn renormalize_all(pool: &SqlitePool) -> AppResult<u64> {
    let mut tx = pool.begin().await?;
    let rows = sqlx::query!(r#"SELECT id AS "id!", raw_label, normalized_label FROM operation"#)
        .fetch_all(&mut *tx)
        .await?;
    let mut updated = 0;
    for row in rows {
        let normalized = normalize_label(&row.raw_label);
        if normalized != row.normalized_label {
            sqlx::query!(
                "UPDATE operation SET normalized_label = ?2 WHERE id = ?1",
                row.id,
                normalized
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "INSERT INTO label_category_memory (normalized_label, category_id, last_used_at)
                 SELECT ?2, category_id, last_used_at FROM label_category_memory
                 WHERE normalized_label = ?1
                 ON CONFLICT (normalized_label) DO UPDATE
                 SET category_id = excluded.category_id, last_used_at = excluded.last_used_at
                 WHERE excluded.last_used_at > label_category_memory.last_used_at",
                row.normalized_label,
                normalized
            )
            .execute(&mut *tx)
            .await?;
            updated += 1;
        }
    }
    tx.commit().await?;
    Ok(updated)
}

#[cfg(test)]
mod tests;
