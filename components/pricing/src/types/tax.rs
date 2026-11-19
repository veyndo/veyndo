//! Tax classes and their destination rules; current tax law is merchant configuration, not bundled tax advice.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaxConfig {
    pub id: String,
    pub rates: HashMap<String, f64>,
    #[serde(default)]
    pub translations: HashMap<String, veyndo_core::localized::Text>,
    #[serde(default)]
    pub default_rate: Option<f64>,
    #[serde(default)]
    pub rules: Vec<DestinationRule>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DestinationRule {
    pub id: String,
    pub country: String,
    pub rate: f64,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub states: Vec<String>,
    #[serde(default)]
    pub postal_codes: Vec<String>,
    #[serde(default)]
    pub postal_prefixes: Vec<String>,
    #[serde(default)]
    pub postal_from: Option<String>,
    #[serde(default)]
    pub postal_to: Option<String>,
    #[serde(default)]
    pub active_from: Option<String>,
    #[serde(default)]
    pub active_until: Option<String>,
    #[serde(default)]
    pub condition: Option<Value>,
}
/// Where a delivery goes, as far as tax rules look at it.
pub struct Destination<'a> {
    pub country: &'a str,
    pub state: &'a str,
    pub postal_code: &'a str,
}
