//! Backward-compatible enrichment of existing configuration with bundled translated method labels.
use crate::types::settings::Settings;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::localized::Text;
use veyndo_core::{Result, bad};

/// Reads stored settings and fills in bundled method labels where none are stored.
pub fn decode(v: Value) -> Result<Settings> {
    serde_json::from_value(v)
        .map(enrich_defaults)
        .map_err(|_| bad("Invalid commerce configuration"))
}

pub fn enrich_defaults(mut s: Settings) -> Settings {
    static DEFAULTS: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    let defaults = DEFAULTS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json"))
            .expect("settings fixture")
    });
    for v in &mut s.shipping {
        if v.translations.is_empty() {
            v.translations = builtin(defaults, "shipping", &v.id, &v.name, &s.main_locale);
        }
    }
    for v in &mut s.payments {
        if v.translations.is_empty() {
            v.translations = builtin(defaults, "payments", &v.id, &v.name, &s.main_locale);
        }
    }
    for v in &mut s.taxes {
        if v.translations.is_empty() {
            v.translations = serde_json::from_value(
                defaults["taxes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|x| x["id"] == v.id)
                    .map(|x| x["translations"].clone())
                    .unwrap_or(json!({})),
            )
            .unwrap_or_default();
        }
    }
    s
}
fn builtin(
    defaults: &Value,
    section: &str,
    id: &str,
    name: &str,
    main: &str,
) -> HashMap<String, Text> {
    if let Some(v) = defaults[section]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == id && v["name"] == name)
    {
        serde_json::from_value(v["translations"].clone()).unwrap_or_default()
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
