//! Backward-compatible enrichment of existing configuration with bundled translated method labels.
use crate::types::settings::Settings;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::document::lenient;
use veyndo_core::localized::Text;
use veyndo_core::{Result, bad};

/// Reads stored settings and fills in bundled method labels where none are stored.
pub fn decode(v: Value) -> Result<Settings> {
    serde_json::from_value(v)
        .map(enrich_defaults)
        .map_err(|_| bad("Invalid commerce configuration"))
}

pub fn enrich_defaults(mut s: Settings) -> Settings {
    static DEFAULTS: std::sync::OnceLock<Bundled> = std::sync::OnceLock::new();
    let defaults = DEFAULTS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json"))
            .expect("settings fixture")
    });
    for v in &mut s.shipping {
        if v.translations.is_empty() {
            v.translations = builtin(&defaults.shipping, &v.id, &v.name, &s.main_locale);
        }
    }
    for v in &mut s.payments {
        if v.translations.is_empty() {
            v.translations = builtin(&defaults.payments, &v.id, &v.name, &s.main_locale);
        }
    }
    for v in &mut s.taxes {
        if v.translations.is_empty() {
            v.translations = serde_json::from_value(
                defaults
                    .taxes
                    .iter()
                    .find(|x| x.id.as_deref() == Some(&v.id))
                    .map(|x| x.translations.clone())
                    .unwrap_or(json!({})),
            )
            .unwrap_or_default();
        }
    }
    s
}
/// The bundled demo methods and tax classes whose labels fill unlabelled stored ones.
#[derive(Deserialize)]
struct Bundled {
    shipping: Vec<BundledRecord>,
    payments: Vec<BundledRecord>,
    taxes: Vec<BundledRecord>,
}

/// A bundled record's identity and translated labels.
#[derive(Deserialize, Default)]
#[serde(default)]
struct BundledRecord {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    translations: Value,
}

fn builtin(records: &[BundledRecord], id: &str, name: &str, main: &str) -> HashMap<String, Text> {
    if let Some(v) = records
        .iter()
        .find(|v| v.id.as_deref() == Some(id) && v.name.as_deref() == Some(name))
    {
        serde_json::from_value(v.translations.clone()).unwrap_or_default()
    } else {
        HashMap::from([(
            main.into(),
            Text {
                name: Some(name.into()),
                description: None,
            },
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, enrich_defaults};
    use crate::types::settings::Settings;
    use serde_json::{Value, json};

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap()
    }

    #[test]
    fn stored_settings_without_labels_get_the_bundled_ones() {
        let mut v = fixture();
        let bundled = v["shipping"][0]["translations"].clone();
        v["shipping"][0]["translations"] = json!({});
        v["payments"][0]["translations"] = json!({});
        v["taxes"][0]["translations"] = json!({});
        let s = decode(v).unwrap();
        assert_eq!(json!(s.shipping[0].translations), bundled);
        assert!(!s.payments[0].translations.is_empty());
        assert!(!s.taxes[0].translations.is_empty());
        assert!(decode(json!({"countries":"DE"})).is_err());
    }

    #[test]
    fn an_own_method_gets_its_name_in_the_main_language_and_stored_labels_stay() {
        let mut v = fixture();
        v["shipping"][0]["name"] = json!("Bike courier");
        v["shipping"][0]["translations"] = json!({});
        v["payments"][0]["translations"] =
            json!({"de-DE":{"name":"Eigene Zahlung","description":null}});
        let s: Settings = serde_json::from_value(v).unwrap();
        let s = enrich_defaults(s);
        assert_eq!(s.shipping[0].translations.len(), 1);
        assert_eq!(
            s.shipping[0].translations[&s.main_locale].name.as_deref(),
            Some("Bike courier")
        );
        assert_eq!(s.payments[0].translations.len(), 1);
        assert_eq!(
            s.payments[0].translations["de-DE"].name.as_deref(),
            Some("Eigene Zahlung")
        );
    }
}
