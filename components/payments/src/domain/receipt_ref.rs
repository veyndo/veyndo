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
