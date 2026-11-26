//! One payment attempt of an order as the ledger stores it, with the typed view of its provider context.
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};

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

/// The provider context fields the ledger reads, typed as it reads them; a missing field is `null` as before.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AttemptContext {
    pub account_ref: Value,
    pub method: Value,
    pub channel: Value,
    pub return_origin: Value,
    pub checkout: Value,
    pub intent: Value,
    pub capabilities: Value,
    pub authorization_id: Value,
    pub captured_amount_minor: Value,
    pub capture_closed: Value,
    pub capture_receipts: Value,
    pub provider_refunds: Value,
    pub vault_status: Value,
    #[serde(deserialize_with = "lenient")]
    pub funding: Funding,
}

/// The funding choice bound to an attempt.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Funding {
    pub save: Value,
    pub consent_id: Value,
    pub instrument_id: Value,
    pub customer_id: Value,
    pub consent_version: Value,
}

impl AttemptContext {
    /// The provider method declares the capability.
    pub fn can(&self, capability: &str) -> bool {
        self.capabilities
            .as_array()
            .is_some_and(|cs| cs.iter().any(|c| c == capability))
    }
}

/// The typed view of a provider context document.
pub fn context_of(context: &Value) -> AttemptContext {
    view(context)
}

impl Attempt {
    /// The typed view of the attempt's provider context.
    pub fn ctx(&self) -> AttemptContext {
        context_of(&self.context)
    }
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

#[cfg(test)]
mod tests {
    use super::context_of;
    use serde_json::json;

    #[test]
    fn the_typed_context_reads_exactly_what_indexing_read() {
        for c in [
            json!({"accountRef":"acct","method":"card","channel":"b2b","intent":"authorize","capabilities":["refund","multi_capture"],"authorizationId":"auth-1","capturedAmountMinor":400,"captureClosed":false,"funding":{"save":true,"customerId":"c-1","instrumentId":"i-1","consentId":"k","consentVersion":"save-payment@1"}}),
            json!({"accountRef":7,"capabilities":"refund","funding":null,"captureClosed":"yes"}),
            json!(null),
            json!([1, 2]),
        ] {
            let t = context_of(&c);
            assert_eq!(t.account_ref, c["accountRef"]);
            assert_eq!(t.method, c["method"]);
            assert_eq!(t.channel, c["channel"]);
            assert_eq!(t.intent, c["intent"]);
            assert_eq!(t.authorization_id, c["authorizationId"]);
            assert_eq!(t.captured_amount_minor, c["capturedAmountMinor"]);
            assert_eq!(t.capture_closed, c["captureClosed"]);
            assert_eq!(t.funding.save, c["funding"]["save"]);
            assert_eq!(t.funding.customer_id, c["funding"]["customerId"]);
            assert_eq!(t.funding.instrument_id, c["funding"]["instrumentId"]);
            assert_eq!(t.funding.consent_version, c["funding"]["consentVersion"]);
            for cap in ["refund", "multi_capture", "vault"] {
                assert_eq!(
                    t.can(cap),
                    c["capabilities"]
                        .as_array()
                        .is_some_and(|cs| cs.iter().any(|v| v == cap))
                );
            }
        }
    }
}
