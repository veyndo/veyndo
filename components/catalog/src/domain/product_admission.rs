//! Admit native product edits and expand explicit category-only changes through the same revisioned editor.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::{Error, Result, bad};
/// A change of category assignments only, against the clock the merchant read.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CategoryEdit {
    pub revision: i64,
    #[serde(default)]
    pub content_revision: Option<i64>,
    pub category_ids: Vec<String>,
}
/// Reads a bounded category-only edit; full product fields may not ride along.
pub fn category_edit(v: &Value) -> Result<CategoryEdit> {
    let edit: CategoryEdit = serde_json::from_value(v.clone())
        .map_err(|_| bad("Category-only product edit requires revision and categoryIds; do not mix full product fields"))?;
    if edit.revision < 0
        || edit.content_revision.is_some_and(|n| n < 0)
        || edit.category_ids.len() > 100
        || edit.category_ids.iter().any(|id| {
            id.is_empty()
                || id.len() > 160
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
        })
        || edit
            .category_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != edit.category_ids.len()
    {
        return Err(bad("Invalid category-only product revision or selection"));
    }
    Ok(edit)
}
/// Admits a product write as either a full edit or an explicit category-only edit.
pub fn admit_product_write(v: &Value, create: bool) -> Result<()> {
    if v.get("categoryIds").is_some() {
        if create {
            return Err(bad("New products require full product metadata"));
        }
        category_edit(v)?;
    } else {
        let edit: crate::types::product_edit::Edit = serde_json::from_value(v.clone())
            .map_err(|_| bad("Product edit requires revision, translations and extra; category-only edits use revision and categoryIds"))?;
        if let Some(catalog) = edit.catalog {
            catalog.validate()?;
        }
        if let Some(commerce) = edit.commerce {
            commerce.validate()?;
        }
    }
    Ok(())
}
/// What an agent must know before it writes a product.
pub fn product_write_guidance() -> Value {
    json!({"categoryOnly":{"example":{"revision":1,"categoryIds":["selected_category"]},"semantics":"Replace only category assignments. Preserve existing category IDs when adding one. Read merchant.product.content first and use its original revision; optionally include its contentRevision. No translations, prices, inventory, media, identity or channel fields are changed. Cannot be combined with full product fields."},"fullEdit":"Read merchant.product.content; preserve revision, translations, extra and untouched catalog fields. Omit commerce for content-only changes. Product creation requires full metadata including catalog. Never invent metadata or refresh a reviewed revision."})
}
/// The full edit that replaces only the category assignments of the product as currently read.
pub fn category_assignment(edit: &CategoryEdit, current: &Value) -> Result<Value> {
    // The native writer checks this original clock again under its product lock.
    // Do not substitute the freshly read revision and silently accept stale input.
    let (clock, expected) = edit
        .content_revision
        .map_or(("revision", edit.revision), |n| ("contentRevision", n));
    if current[clock].as_i64() != Some(expected) {
        return Err(Error::revision_at(
            current[clock].as_i64().unwrap_or(0),
            &format!("/{clock}"),
            "Product changed",
        ));
    }
    let mut result = json!({"revision":edit.revision,"translations":current["translations"],"extra":current["extra"],"catalog":current["catalog"]});
    if let Some(revision) = edit.content_revision {
        result["contentRevision"] = json!(revision);
    }
    result["catalog"]["categoryIds"] = json!(edit.category_ids);
    // Keep channel writes out of an assignment-only operation as well as commerce.
    if let Some(catalog) = result["catalog"].as_object_mut() {
        catalog.remove("salesChannelIds");
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::{admit_product_write, category_assignment, category_edit};
    use serde_json::json;
    #[test]
    fn category_only_edits_are_explicit_and_bounded() {
        assert!(
            admit_product_write(&json!({"revision":2,"categoryIds":["collection"]}), false).is_ok()
        );
        for v in [
            json!({"revision":2}),
            json!({"revision":2,"categoryIds":["collection"],"commerce":{"stock":999}}),
            json!({"revision":2,"categoryIds":["x","x"]}),
            json!({"revision":-1,"categoryIds":[]}),
            json!({"revision":2,"categoryIds":["../foreign"]}),
        ] {
            assert!(admit_product_write(&v, false).is_err(), "{v}");
        }
        assert!(admit_product_write(&json!({"revision":2,"categoryIds":[]}), true).is_err());
    }
    #[test]
    fn category_assignment_keeps_the_read_clock_and_drops_channels() {
        let current = json!({"revision":2,"contentRevision":5,"translations":{},"extra":{},
            "catalog":{"categoryIds":["old"],"salesChannelIds":["web"]}});
        let edit = category_edit(&json!({"revision":2,"categoryIds":["new"]})).unwrap();
        let full = category_assignment(&edit, &current).unwrap();
        assert_eq!(full["catalog"]["categoryIds"], json!(["new"]));
        assert!(full["catalog"].get("salesChannelIds").is_none());
        assert_eq!(full["revision"], 2);
        let stale = category_edit(&json!({"revision":1,"categoryIds":["new"]})).unwrap();
        assert!(category_assignment(&stale, &current).is_err());
        let content =
            category_edit(&json!({"revision":2,"contentRevision":4,"categoryIds":[]})).unwrap();
        assert!(category_assignment(&content, &current).is_err());
    }
}
