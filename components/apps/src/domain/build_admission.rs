//! What an App Studio build must satisfy before it is stored: no reserved or reviewed-only package, bounded labels, the shop main language.
use crate::domain::manifest_validation;
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad};

/// Admits a package App Studio may build: valid, not a reserved app, and without Wasm configuration that needs a reviewed package.
pub fn validate(m: &Manifest, publishers: &Value) -> Result<()> {
    manifest_validation::validate(m, publishers)?;
    if [
        "engraving",
        "paypal",
        "shopware_payments",
        "storyfront",
        "google_analytics",
        "gmail",
        "slack",
        "email",
    ]
    .contains(&m.id.as_str())
        || m.configuration.as_ref().is_some_and(|c| c.inputs.is_none())
        || m.slots.iter().any(|s| {
            !matches!(s.component.as_str(), "entity-list" | "entity-form")
                && !(s.component == "product-configuration"
                    && m.configuration.as_ref().is_some_and(|c| c.inputs.is_some()))
        })
    {
        return Err(bad(
            "Reserved apps and Wasm configuration require a separate reviewed package; service code remains operator deployed",
        ));
    }
    let labels = std::iter::once(&m.name)
        .chain(m.entities.iter().map(|e| &e.label))
        .chain(
            m.entities
                .iter()
                .flat_map(|e| e.fields.iter().map(|f| &f.label)),
        )
        .chain(m.slots.iter().map(|s| &s.label))
        .chain(m.surfaces.iter().map(|s| &s.label));
    if labels
        .into_iter()
        .any(|l| l.is_empty() || l.values().any(|s| s.trim().is_empty() || s.len() > 100))
    {
        return Err(bad("App and field labels must be nonempty and bounded"));
    }
    Ok(())
}

/// Requires every label and the summary in the shop main language; other languages inherit from it.
pub fn main_language(m: &Manifest, summary: &Value, main: &str) -> Result<()> {
    let base = main.split('-').next().unwrap_or(main);
    let labels = std::iter::once(&m.name)
        .chain(m.entities.iter().map(|e| &e.label))
        .chain(
            m.entities
                .iter()
                .flat_map(|e| e.fields.iter().map(|f| &f.label)),
        )
        .chain(m.surfaces.iter().map(|s| &s.label))
        .chain(
            m.views
                .iter()
                .flat_map(|v| v.blocks.iter().map(|b| &b.title)),
        )
        .chain(m.capability.iter().map(|c| &c.intent))
        .chain(m.capability.iter().flat_map(|c| &c.assumptions))
        .chain(
            m.capability
                .iter()
                .flat_map(|c| c.examples.iter().map(|e| &e.label)),
        );
    if labels.into_iter().any(|l| {
        l.get(main)
            .or_else(|| l.get(base))
            .is_none_or(|s| s.trim().is_empty())
    }) || summary.as_object().is_none_or(|o| {
        o.values()
            .any(|s| s.as_str().is_none_or(|s| s.len() > 1000))
    }) || summary[main]
        .as_str()
        .or_else(|| summary[base].as_str())
        .is_none_or(|s| s.trim().is_empty())
    {
        return Err(bad(
            "App labels and summary need the shop main language; other languages inherit",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{main_language, validate};
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::hash;

    fn advisor() -> Manifest {
        serde_json::from_str(include_str!(
            "../../../../extensions/apps/product-advisor/manifest.json"
        ))
        .unwrap()
    }

    #[test]
    fn development_digest_preserves_native_serialized_bytes() {
        let value = json!(advisor());
        assert_eq!(
            veyndo_app_storage::build_versions::digest(&value),
            hash(&value.to_string())
        );
    }

    #[test]
    fn developer_rejects_reserved_apps_and_unbounded_labels() {
        let reserved: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/storyfront/manifest.json"
        ))
        .unwrap();
        assert!(validate(&reserved, &json!({})).is_err());
        let mut renamed = advisor();
        validate(&renamed, &json!({})).unwrap();
        renamed.id = "gmail".into();
        assert!(validate(&renamed, &json!({})).is_err());
        let mut unlabelled = advisor();
        unlabelled.name.clear();
        assert!(validate(&unlabelled, &json!({})).is_err());
        let mut long = advisor();
        for label in long.name.values_mut() {
            *label = "x".repeat(101);
        }
        assert!(validate(&long, &json!({})).is_err());
    }

    #[test]
    fn labels_and_summary_need_the_shop_main_language() {
        let m = advisor();
        let main = m.name.keys().next().unwrap().clone();
        let summary = json!({ main.as_str(): "Advises shoppers" });
        main_language(&m, &summary, &main).unwrap();
        assert!(main_language(&m, &summary, "zz-ZZ").is_err());
        assert!(main_language(&m, &json!({}), &main).is_err());
        assert!(main_language(&m, &json!({ main.as_str(): "  " }), &main).is_err());
        assert!(main_language(&m, &json!({ main.as_str(): "x".repeat(1001) }), &main).is_err());
        assert!(main_language(&m, &json!("text"), &main).is_err());
        let mut partial = m.clone();
        partial.name.remove(&main);
        assert!(main_language(&partial, &summary, &main).is_err());
    }
}
