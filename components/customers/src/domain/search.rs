//! A merchant's search text as a literal, case-insensitive LIKE fragment.

/// Lower-cases and escapes the text so `%`, `_` and `\\` match themselves.
pub fn literal(query: &str) -> String {
    query
        .trim()
        .to_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::literal;

    #[test]
    fn wildcards_are_searched_literally() {
        assert_eq!(literal("  Ann_50%\\x "), "ann\\_50\\%\\\\x");
        assert_eq!(literal(""), "");
    }
}
