//! Label normalization and category memory.

/// Normalized label used for categorization and payees.
///
/// Temporary version: uppercase with collapsed whitespace. The bank-specific cleanup
/// (prefixes, dates, card numbers, references) comes with V1-03.3.
pub fn normalize_label(raw_label: &str) -> String {
    raw_label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercases_and_collapses_whitespace() {
        assert_eq!(
            normalize_label("  Boulangerie   Martin "),
            "BOULANGERIE MARTIN"
        );
        assert_eq!(normalize_label("Café\tdu coin"), "CAFÉ DU COIN");
    }
}
