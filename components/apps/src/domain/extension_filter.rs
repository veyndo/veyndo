//! Exact app-field predicates: a bounded, typed wire form that is bound to the current package before use.
use serde::Deserialize;
use serde_json::Value;
use veyndo_app_model::declarations::identifier;
use veyndo_core::{Result, bad};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
/// One exact predicate on an indexed app field.
pub struct Filter {
    pub app: String,
    pub entity: String,
    pub field: String,
    pub value: Value,
    #[serde(skip)]
    pub(crate) anchor: String,
    #[serde(skip)]
    pub(crate) digest: String,
}

/// Reads 1 to 4 exact filters from at most 4096 bytes.
pub fn decode(input: &str) -> Result<Vec<Filter>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if input.len() > 4096 {
        return Err(bad("App filter exceeds 4096 bytes"));
    }
    let filters: Vec<Filter> =
        serde_json::from_str(input).map_err(|_| bad("Invalid app filter"))?;
    if filters.is_empty()
        || filters.len() > 4
        || filters.iter().any(|f| {
            [&f.app, &f.entity, &f.field].iter().any(|s| !identifier(s))
                || !(f.value.is_string() || f.value.is_i64() || f.value.is_boolean())
                || f.value.as_str().is_some_and(|s| s.len() > 2000)
        })
    {
        return Err(bad("Use 1..4 exact indexed app-field filters"));
    }
    Ok(filters)
}
impl Filter {
    /// Binds the filter to the anchor field and the digest of the package it was admitted against.
    pub fn bind(&mut self, anchor: String, digest: String) {
        self.anchor = anchor;
        self.digest = digest;
    }
}

#[cfg(test)]
mod tests {
    use super::decode;
    #[test]
    fn no_identity_injection_or_unbounded_or_untyped_filter() {
        for value in [
            "{}",
            "[]",
            r#"[{"app":"a","entity":"b","field":"x","value":null}]"#,
            r#"[{"app":"a","entity":"b","field":"x","value":[],"tenant":"other"}]"#,
            r#"[{"app":"a;DROP","entity":"b","field":"x","value":"s"}]"#,
        ] {
            assert!(decode(value).is_err());
        }
        assert_eq!(
            decode(r#"[{"app":"a","entity":"b","field":"x","value":false}]"#)
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn a_bound_filter_keeps_the_anchor_and_package_digest() {
        let mut filters = decode(r#"[{"app":"a","entity":"b","field":"x","value":"s"}]"#).unwrap();
        filters[0].bind("owner".into(), "digest".into());
        assert_eq!(filters[0].anchor, "owner");
        assert_eq!(filters[0].digest, "digest");
    }
}
