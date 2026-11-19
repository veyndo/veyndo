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
    use super::parse;
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
}
