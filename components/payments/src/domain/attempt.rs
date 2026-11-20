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

/// An authorized attempt over ten euros for the tests of this component.
#[cfg(test)]
pub(crate) fn sample() -> Attempt {
    Attempt {
        id: "attempt-1".into(),
        provider: "example".into(),
        context: serde_json::json!({"accountRef":"account-1"}),
        tenant: "shop".into(),
        order: "order-1".into(),
        amount: 1000,
        currency: "EUR".into(),
        currency_scale: 2,
        state: "authorized".into(),
        provider_order: None,
        capture: None,
        refunded: 0,
        adapter_version: "1.3.0".into(),
        environment: "simulation".into(),
        bn_code: String::new(),
    }
}
