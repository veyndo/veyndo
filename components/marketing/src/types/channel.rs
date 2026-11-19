//! The sales channel record and the POS location it may carry; one tenant, independent visibility and languages.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
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
pub fn normalize_channel(data: &mut Value, id: &str, settings: &Settings) {
    if id == "default" {
        data["locales"] = json!(settings.locales);
    }
}
