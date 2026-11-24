//! Receipt identity: a provider answer must name exactly the attempt, account and invoice it was issued for.
use crate::domain::attempt::Attempt;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{Raw, lenient, view};
use veyndo_core::{Result, bad};

/// A provider adapter's receipt or notification as the ledger reads it; a field of another shape reads as absent.
#[derive(Deserialize, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderReceipt {
    #[serde(deserialize_with = "lenient")]
    pub api_version: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub provider: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub adapter_version: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub tenant: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub attempt_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub order_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub environment: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub status: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub event_id: Option<String>,
    pub account_ref: Value,
    pub reference: Value,
    pub authorization_id: Value,
    pub capture_id: Value,
    pub approval_url: Value,
    #[serde(deserialize_with = "lenient")]
    pub amount_minor: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub authorized_amount_minor: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub settled_amount_minor: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub confirmed: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub capture_pending: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub capture_closed: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub refund: Raw<RefundReceipt>,
}

/// The refund a receipt confirms.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct RefundReceipt {
    #[serde(deserialize_with = "lenient")]
    pub amount_minor: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub currency: Option<String>,
}

impl ProviderReceipt {
    /// Whether the provider confirmed this receipt.
    pub fn confirmed(&self) -> bool {
        self.confirmed == Some(true)
    }
}

/// Whether an optional text field holds exactly .
pub fn is(field: &Option<String>, expected: &str) -> bool {
    field.as_deref() == Some(expected)
}

pub fn identity(p: &Attempt, v: &Value) -> Result<()> {
    let r: ProviderReceipt = view(v);
    if !is(&r.api_version, "1")
        || !is(&r.provider, &p.provider)
        || !is(&r.adapter_version, &p.adapter_version)
        || !is(&r.tenant, &p.tenant)
        || !is(&r.attempt_id, &p.id)
        || !is(&r.order_id, &p.order)
        || !is(&r.environment, &p.environment)
        || r.account_ref != p.context["accountRef"]
        || !is(&r.currency, &p.currency)
        || r.amount_minor != Some(p.amount)
        || p.provider_order.as_ref().is_some_and(|x| r.reference != *x)
    {
        return Err(bad("Payment receipt identity, account or invoice mismatch"));
    }
    Ok(())
}
pub fn reference(v: &Value) -> Result<&str> {
    v.as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 200
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-:.".contains(&b))
        })
        .ok_or(bad("Invalid provider receipt reference"))
}

#[cfg(test)]
mod tests {
    use super::{identity, reference};
    use crate::domain::attempt::sample;
    use serde_json::{Value, json};

    fn receipt() -> Value {
        json!({"apiVersion":"1","provider":"example","adapterVersion":"1.3.0","tenant":"shop",
            "attemptId":"attempt-1","orderId":"order-1","environment":"simulation",
            "accountRef":"account-1","currency":"EUR","amountMinor":1000,"reference":"ref-1"})
    }

    #[test]
    fn a_receipt_names_exactly_its_attempt_account_and_amount() {
        let p = sample();
        assert!(identity(&p, &receipt()).is_ok());
        for (key, value) in [
            ("apiVersion", json!("2")),
            ("provider", json!("other")),
            ("adapterVersion", json!("1.2.0")),
            ("tenant", json!("other-shop")),
            ("attemptId", json!("attempt-2")),
            ("orderId", json!("order-2")),
            ("environment", json!("live")),
            ("accountRef", json!("account-2")),
            ("currency", json!("USD")),
            ("amountMinor", json!(1001)),
            ("amountMinor", json!("1000")),
        ] {
            let mut changed = receipt();
            changed[key] = value;
            assert!(identity(&p, &changed).is_err(), "{key}");
        }
    }

    #[test]
    fn a_known_provider_order_binds_the_receipt_reference() {
        let mut p = sample();
        p.provider_order = Some("ref-1".into());
        assert!(identity(&p, &receipt()).is_ok());
        p.provider_order = Some("ref-2".into());
        assert!(identity(&p, &receipt()).is_err());
    }

    #[test]
    fn references_are_short_plain_identifiers() {
        assert_eq!(reference(&json!("CAP-1_a:b.c")).ok(), Some("CAP-1_a:b.c"));
        for invalid in [
            json!(""),
            json!("a b"),
            json!("a/b"),
            json!(7),
            Value::Null,
            json!("x".repeat(201)),
        ] {
            assert!(reference(&invalid).is_err(), "{invalid}");
        }
        assert!(reference(&json!("x".repeat(200))).is_ok());
    }
}
