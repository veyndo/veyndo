//! Canonical import references resolve to tenant-bound native IDs; the existing product owner validates every relation.
use crate::types::source_record::SourceRecord;
use serde_json::{Value, json};
use veyndo_core::{Result, bad};
pub fn identity(r: &SourceRecord, t: &str, kind: &str, source_id: &str) -> Result<String> {
    if source_id.is_empty() || source_id.len() > 200 || source_id.chars().any(char::is_control) {
        return Err(bad("Invalid source relation identity"));
    }
    Ok(SourceRecord {
        version: r.version,
        source: r.source.clone(),
        source_shop: r.source_shop.clone(),
        kind: kind.into(),
        source_id: source_id.into(),
        data: json!({}),
        warnings: vec![],
    }
    .identity(t))
}
pub fn product(r: &SourceRecord, t: &str) -> Result<Value> {
    let parent = r
        .data
        .get("parentSourceId")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_str()
                .ok_or(bad("Invalid source parent identity"))
                .and_then(|s| identity(r, t, "products", s))
        })
        .transpose()?;
    let mut categories = vec![];
    if let Some(values) = r.data.get("categorySourceIds") {
        let values = values
            .as_array()
            .filter(|xs| xs.len() <= 100)
            .ok_or(bad("Invalid source category references"))?;
        for value in values {
            categories.push(identity(
                r,
                t,
                "categories",
                value
                    .as_str()
                    .ok_or(bad("Category source ID must be text"))?,
            )?);
        }
    }
    if categories.is_empty() {
        categories.push("catalog-root".into());
    }
    let options = r.data.get("options").cloned().unwrap_or(json!({}));
    if !options.is_object() {
        return Err(bad("Variant options must be a canonical object"));
    }
    Ok(json!({"parentId":parent,"categoryIds":categories,"options":options}))
}

#[cfg(test)]
mod tests {
    use super::{identity, product};
    use crate::types::source_record::SourceRecord;
    use serde_json::{Value, json};

    fn record(data: Value) -> SourceRecord {
        SourceRecord {
            version: 1,
            source: "shopware".into(),
            source_shop: "old-shop".into(),
            kind: "products".into(),
            source_id: "p1".into(),
            data,
            warnings: vec![],
        }
    }

    #[test]
    fn a_relation_identity_is_bound_to_shop_kind_and_source_id() {
        let r = record(json!({}));
        let id = identity(&r, "shop", "categories", "c1").unwrap();
        assert_eq!(
            identity(&r, "shop", "categories", "c1").ok(),
            Some(id.clone())
        );
        assert_ne!(
            identity(&r, "shop", "products", "c1").ok(),
            Some(id.clone())
        );
        assert_ne!(
            identity(&r, "other-shop", "categories", "c1").ok(),
            Some(id.clone())
        );
        assert_ne!(identity(&r, "shop", "categories", "c2").ok(), Some(id));
        assert_eq!(
            identity(&r, "shop", "products", "p1").ok(),
            Some(r.identity("shop"))
        );
        for invalid in ["", "a\nb", &"x".repeat(201)] {
            assert!(identity(&r, "shop", "categories", invalid).is_err());
        }
    }

    #[test]
    fn product_relations_resolve_parent_and_categories_or_the_catalogue_root() {
        let r = record(
            json!({"parentSourceId":"parent","categorySourceIds":["c1","c2"],"options":{"size":"M"}}),
        );
        let v = product(&r, "shop").unwrap();
        assert_eq!(
            v["parentId"],
            json!(identity(&r, "shop", "products", "parent").unwrap())
        );
        assert_eq!(
            v["categoryIds"],
            json!([
                identity(&r, "shop", "categories", "c1").unwrap(),
                identity(&r, "shop", "categories", "c2").unwrap()
            ])
        );
        assert_eq!(v["options"], json!({"size":"M"}));
        let plain = product(&record(json!({"parentSourceId":null})), "shop").unwrap();
        assert_eq!(
            plain,
            json!({"parentId":null,"categoryIds":["catalog-root"],"options":{}})
        );
        for invalid in [
            json!({"parentSourceId":7}),
            json!({"categorySourceIds":"c1"}),
            json!({"categorySourceIds":[7]}),
            json!({"categorySourceIds":vec!["c"; 101]}),
            json!({"options":["M"]}),
        ] {
            assert!(
                product(&record(invalid.clone()), "shop").is_err(),
                "{invalid}"
            );
        }
    }
}
