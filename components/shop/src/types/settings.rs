//! Checkout selection and configuration data contracts.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use veyndo_core::localized::{Languages, Text};
pub use veyndo_pricing::customer::Address;
use veyndo_pricing::types::customer_group::{CustomerGroup, priced_net};
pub use veyndo_pricing::types::tax::TaxConfig;
use veyndo_shipping::domain::geography::Country;

/// Whether a buyer wants to save a new payment method or pay with a saved one.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FundingChoice {
    #[serde(default)]
    pub save: bool,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckoutSelection {
    #[serde(default = "country_default")]
    pub country: String,
    #[serde(default = "shipping_default")]
    pub shipping_method_id: String,
    #[serde(default = "payment_default")]
    pub payment_method_id: String,
    #[serde(default)]
    pub address: Option<Address>,
    #[serde(default)]
    pub billing_address: Option<Address>,
    #[serde(default)]
    pub shipping_address_id: Option<String>,
    #[serde(default)]
    pub billing_address_id: Option<String>,
    #[serde(default)]
    pub customer_email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_funding: Option<FundingChoice>,
}
pub fn country_default() -> String {
    veyndo_pricing::customer::country_default()
}
pub fn shipping_default() -> String {
    "pickup".into()
}
pub fn payment_default() -> String {
    "demo-card".into()
}
impl CheckoutSelection {
    pub fn defaults() -> Self {
        Self {
            country: country_default(),
            shipping_method_id: shipping_default(),
            payment_method_id: payment_default(),
            address: None,
            billing_address: None,
            shipping_address_id: None,
            billing_address_id: None,
            customer_email: None,
            payment_funding: None,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub b2b: crate::types::b2b::Config,
    #[serde(default)]
    pub ai_policy: crate::types::guardrails::Config,
    #[serde(default)]
    pub supply: veyndo_inventory::domain::supply::Config,
    #[serde(default)]
    pub legal: crate::types::legal::Config,
    #[serde(default)]
    pub currencies: veyndo_pricing::types::currency::Config,
    #[serde(default = "main_locale")]
    pub main_locale: String,
    #[serde(default = "content_locales")]
    pub locales: Vec<String>,
    #[serde(default)]
    pub country_definitions: Vec<Country>,
    pub countries: Vec<String>,
    #[serde(default = "crate::domain::customer_groups::defaults")]
    pub customer_groups: Vec<CustomerGroup>,
    pub taxes: Vec<TaxConfig>,
    pub shipping: Vec<Shipping>,
    pub payments: Vec<Payment>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shipping {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub translations: HashMap<String, Text>,
    pub price: f64,
    pub free_above: Option<f64>,
    pub min_days: i32,
    pub max_days: i32,
    pub countries: Vec<String>,
    pub active: bool,
    pub tax_type: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Payment {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub translations: HashMap<String, Text>,
    #[serde(default)]
    pub countries: Vec<String>,
    #[serde(default)]
    pub restricted_countries: bool,
    pub active: bool,
    pub business_only: bool,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_mode: Option<String>,
}
pub fn main_locale() -> String {
    "en-GB".into()
}
pub fn content_locales() -> Vec<String> {
    ["en-GB", "de-DE", "es-ES", "fr-FR"]
        .map(String::from)
        .to_vec()
}

impl Payment {
    pub fn available_in(&self, country: &str) -> bool {
        (!self.restricted_countries && self.countries.is_empty())
            || self.countries.iter().any(|c| c == country)
    }
}

impl Settings {
    /// The enabled content languages; the rules on them are shared by every component.
    pub fn languages(&self) -> Languages<'_> {
        Languages {
            main: &self.main_locale,
            enabled: &self.locales,
        }
    }
    /// Whether the customer group is priced net.
    pub fn is_business(&self, group: &str) -> bool {
        priced_net(&self.customer_groups, group)
    }
}
