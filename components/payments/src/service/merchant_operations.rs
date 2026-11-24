//! The shared merchant payment operation bridge: HTTP, MCP and Flows reach the same native rights and durable jobs.
use crate::ports::{Gateway, Host};
use crate::service::{details, instructions, providers};
use crate::store::commands::enqueue;
use serde_json::Value;
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};

/// Merchant operations a model or a durable caller may request; every effect needs explicit approval.
pub const OPERATIONS: [&str; 6] = [
    "capture",
    "authorize",
    "void",
    "refund",
    "reconcile",
    "cancel",
];

/// The merchant payment transaction projection after the native read right.
pub async fn details(
    gateway: &dyn Gateway,
    a: &State,
    h: &RequestContext,
    id: &str,
) -> Result<Value> {
    let tenant = merchant(h)?;
    permit(h, "payments.read")?;
    details::read(gateway, a, &tenant, id).await
}

/// One merchant payment operation by name: instructions, details, providers, onboarding or an approved ledger command.
pub async fn invoke(
    ports: (&dyn Host, &dyn Gateway),
    a: &State,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    let id = || v["id"].as_str().ok_or(bad("id required"));
    match name {
        "merchant.payment.instructions" => instructions::merchant_read(ports.1, a, h, id()?).await,
        "merchant.payment.details" => details(ports.1, a, h, id()?).await,
        "merchant.payment.providers" => providers::providers(a, h).await,
        "merchant.payment.onboarding" => providers::onboarding(ports, a, h, id()?, v).await,
        "merchant.payment" => {
            let op = v["operation"].as_str().ok_or(bad("operation required"))?;
            if !admitted(op, v) {
                return Err(bad("Payment operation and approval required"));
            }
            enqueue(
                a,
                h,
                id()?,
                op,
                v["requestKey"].as_str().ok_or(bad("requestKey required"))?,
                v,
            )
            .await
        }
        _ => Err(bad("Unknown merchant payment operation")),
    }
}

/// A merchant ledger command names a supported operation and carries `approve=true`.
pub fn admitted(op: &str, v: &Value) -> bool {
    OPERATIONS.contains(&op) && v["approve"] == true
}

#[cfg(test)]
mod tests {
    use super::admitted;
    use serde_json::json;

    #[test]
    fn ledger_commands_need_a_known_operation_and_approval() {
        assert!(admitted("refund", &json!({"approve":true})));
        assert!(!admitted("refund", &json!({"approve":"true"})));
        assert!(!admitted("delete", &json!({"approve":true})));
    }
}
