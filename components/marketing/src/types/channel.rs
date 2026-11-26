//! The sales channel record and the POS location it may carry; one tenant, independent visibility and languages.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::document::merge;
use veyndo_core::{Result, bad};
use veyndo_shop::types::settings::Settings;
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Channel {
    pub name: HashMap<String, String>,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<PosConfig>,
    pub active: bool,
    #[serde(default = "public_visibility")]
    pub visibility: String,
    pub locales: Vec<String>,
    pub product_ids: Vec<String>,
    #[serde(default)]
    pub navigation_category_id: Option<String>,
}
fn public_visibility() -> String {
    "public".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PosConfig {
    pub country: String,
    #[serde(default = "columns")]
    pub columns: u8,
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub receipt_message: HashMap<String, String>,
}
fn columns() -> u8 {
    4
}
pub fn validate_pos(c: &Channel, s: &Settings) -> Result<()> {
    if c.kind != "pos" {
        if c.pos.is_some() {
            return Err(bad("POS configuration requires a POS channel"));
        }
        return Ok(());
    }
    let cfg = c.pos.as_ref().ok_or(bad("POS location country required"))?;
    if !s.countries.contains(&cfg.country) || !(2..=6).contains(&cfg.columns) {
        return Err(bad("Invalid POS country or grid columns"));
    }
    if !cfg.receipt_message.is_empty() {
        s.languages()
            .validate_names(&json!(cfg.receipt_message), 500)?;
    }
    Ok(())
}
/// The main channel always inherits the shop languages.
pub fn normalize_channel(data: &mut Value, id: &str, settings: &Settings) -> Result<()> {
    if id == "default" {
        merge(data, json!({ "locales": settings.locales }))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normalize_channel;
    use serde_json::json;
    use veyndo_shop::types::settings::Settings;

    #[test]
    fn the_main_channel_takes_the_shop_languages_as_indexing_wrote_them() {
        let settings: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        for (id, data) in [
            ("default", json!({"name":{"en":"Main"},"locales":["xx"]})),
            ("default", json!(null)),
            ("other", json!({"locales":["xx"]})),
        ] {
            let mut old = data.clone();
            if id == "default" {
                old["locales"] = json!(settings.locales);
            }
            let mut new = data;
            normalize_channel(&mut new, id, &settings).unwrap();
            assert_eq!(new, old, "{id}");
        }
    }
}
