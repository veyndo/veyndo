//! Explicit product selection is bounded and unique before any translation is queued.
use serde_json::Value;
use veyndo_core::{Result, bad};

/// `None` means the whole catalogue; an explicit selection must name 1..1000 distinct products.
pub fn parse(value: Option<&Value>) -> Result<Option<Vec<String>>> {
    let Some(value) = value else { return Ok(None) };
    let ids: Vec<String> = serde_json::from_value(value.clone())
        .map_err(|_| bad("Product selection must be an array of identifiers"))?;
    validate(&ids)?;
    Ok(Some(ids))
}
pub fn validate(ids: &[String]) -> Result<()> {
    let unique = ids.iter().collect::<std::collections::HashSet<_>>();
    if ids.is_empty()
        || ids.len() > 1000
        || unique.len() != ids.len()
        || ids.iter().any(|id| id.is_empty() || id.len() > 160)
    {
        return Err(bad("Select 1..1000 distinct product identifiers"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{parse, validate};
    use serde_json::json;
    #[test]
    fn missing_means_catalogue_but_explicit_empty_never_does() {
        assert!(parse(None).unwrap().is_none());
        for value in [
            json!([]),
            json!(null),
            json!(["a", "a"]),
            json!(["a", 3]),
            json!([""]),
        ] {
            assert!(parse(Some(&value)).is_err());
        }
        assert_eq!(
            parse(Some(&json!(["b", "a"]))).unwrap().unwrap(),
            vec!["b", "a"]
        );
    }
    #[test]
    fn a_selection_names_at_most_a_thousand_short_distinct_identifiers() {
        let ids = |n: usize| (0..n).map(|i| format!("p{i}")).collect::<Vec<_>>();
        assert!(validate(&ids(1)).is_ok());
        assert!(validate(&ids(1000)).is_ok());
        assert!(validate(&ids(1001)).is_err());
        assert!(validate(&[]).is_err());
        assert!(validate(&["x".repeat(160)]).is_ok());
        assert!(validate(&["x".repeat(161)]).is_err());
        assert!(validate(&["a".into(), "a".into()]).is_err());
    }
}
