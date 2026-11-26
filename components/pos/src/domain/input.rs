//! Strict input envelope shared by HTTP and MCP cashier commands.
use serde::Deserialize;
use serde_json::Value;
use veyndo_pricing::cart::Item;
use veyndo_shop::types::settings::CheckoutSelection;
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub channel_id: Option<String>,
    pub cart_token: Option<String>,
    pub revision: Option<i64>,
    pub items: Option<Vec<Item>>,
    pub checkout: Option<CheckoutSelection>,
    pub query: Option<String>,
    pub after: Option<String>,
    pub barcode: Option<String>,
    pub request_key: Option<String>,
    pub tender_minor: Option<String>,
    pub total_minor: Option<String>,
    pub product_id: Option<String>,
    pub customer_id: Option<String>,
    #[serde(rename = "discount")]
    pub _discount: Option<Value>,
    pub receipt_id: Option<String>,
    pub order_id: Option<String>,
    pub policy_version: Option<String>,
    pub currency: Option<String>,
    pub codes: Option<Vec<String>>,
    pub terms: Option<bool>,
    pub digital_immediate: Option<bool>,
}
