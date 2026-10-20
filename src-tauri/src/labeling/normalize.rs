//! Normalized labels: the key used for categorization and payees.
//!
//! The raw bank label is uppercased, then cleaned of what changes from one operation to the
//! next for the same payee: bank prefixes (`CB`, `PRLV SEPA`, `VIR`…), dates, card numbers,
//! IBANs and references. `raw_label` itself is never modified.

use std::sync::LazyLock;

use regex::Regex;
use sqlx::SqlitePool;

use crate::error::AppResult;

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("normalization patterns are valid")
}

/// Noise removed anywhere in the label, in this order.
static NOISE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
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
    .into_iter()
    .map(regex)
    .collect()
});

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
    let upper = collapse(&raw_label.to_uppercase());

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
            updated += 1;
        }
    }
    tx.commit().await?;
    Ok(updated)
}

#[cfg(test)]
mod tests;
