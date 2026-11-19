//! One payment attempt of an order as the ledger stores it.
use serde_json::Value;

#[derive(Clone)]
pub struct Attempt {
    pub id: String,
    pub provider: String,
    pub context: Value,
    pub tenant: String,
    pub order: String,
    pub amount: i64,
    pub currency: String,
    pub currency_scale: i16,
    pub state: String,
    pub provider_order: Option<String>,
    pub capture: Option<String>,
    pub refunded: i64,
    pub adapter_version: String,
    pub environment: String,
    pub bn_code: String,
}
