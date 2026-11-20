//! Shared relation projections are maintained transactionally by app_record_project; wire values remain bounded ordered arrays.
use serde_json::Value;

/// Whether a relation value is a list of at most 100 distinct short identifiers.
pub fn valid(value: &Value) -> bool {
    value.as_array().is_some_and(|ids| {
        ids.len() <= 100
            && ids
                .iter()
                .all(|id| id.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 100))
            && ids.iter().collect::<std::collections::HashSet<_>>().len() == ids.len()
    })
}

#[cfg(test)]
mod tests {
    use super::valid;
    use serde_json::json;

    #[test]
    fn relations_are_bounded_lists_of_distinct_identifiers() {
        assert!(valid(&json!([])));
        assert!(valid(&json!(["a", "b"])));
        assert!(valid(&json!(vec!["x".repeat(100)])));
        for invalid in [
            json!("a"),
            json!(["a", "a"]),
            json!([""]),
            json!([7]),
            json!(vec!["x".repeat(101)]),
            json!((0..101).map(|i| i.to_string()).collect::<Vec<_>>()),
        ] {
            assert!(!valid(&invalid), "{invalid}");
        }
    }
}
