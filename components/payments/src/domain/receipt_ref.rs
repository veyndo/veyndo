//! Receipt identity: a provider answer must name exactly the attempt, account and invoice it was issued for.
use crate::domain::attempt::Attempt;
use serde_json::Value;
use veyndo_core::{Result, bad};

pub fn identity(p: &Attempt, v: &Value) -> Result<()> {
    if v["apiVersion"] != "1"
        || v["provider"] != p.provider
        || v["adapterVersion"] != p.adapter_version
        || v["tenant"] != p.tenant
        || v["attemptId"] != p.id
        || v["orderId"] != p.order
        || v["environment"] != p.environment
        || v["accountRef"] != p.context["accountRef"]
        || v["currency"] != p.currency
        || v["amountMinor"].as_i64() != Some(p.amount)
        || p.provider_order
            .as_ref()
            .is_some_and(|r| v["reference"] != *r)
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
