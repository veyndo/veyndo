//! App/Flow/MCP payment actions enqueue core jobs; provider-bound attempts prevent cross-app command authority.
use crate::store::commands::{enqueue, order_attempt, provider_owns};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};

/// Queues an approved command for an attempt the calling provider owns.
/// A payment command an app sends: the attempt (or the order event naming it), the operation and its approval.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PaymentCommand {
    #[serde(deserialize_with = "lenient")]
    attempt_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    event: CommandEvent,
    #[serde(deserialize_with = "lenient")]
    operation: Option<String>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    request_key: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CommandEvent {
    #[serde(deserialize_with = "lenient")]
    order_id: Option<String>,
}

pub async fn app_command(
    a: &State,
    h: &RequestContext,
    provider: &str,
    v: &Value,
) -> Result<Value> {
    let t = tenant(h)?;
    let command: PaymentCommand = view(v);
    let attempt_id = if let Some(id) = command.attempt_id.clone() {
        id
    } else {
        let order = command
            .event
            .order_id
            .as_deref()
            .ok_or(bad("Payment action requires attemptId or an order event"))?;
        order_attempt(a, &t, order).await?.ok_or(Error(
            StatusCode::NOT_FOUND,
            "Order payment not found".into(),
        ))?
    };
    if !provider_owns(a, &t, &attempt_id, provider).await? {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Provider payment not found".into(),
        ));
    }
    let op = command
        .operation
        .as_deref()
        .ok_or(bad("Payment operation required"))?;
    if ![
        "capture",
        "authorize",
        "void",
        "cancel",
        "refund",
        "reconcile",
    ]
    .contains(&op)
        || command.approve != Some(true)
    {
        return Err(bad("Explicit approved payment command required"));
    }
    let mut input = json!({});
    for field in ["amountMinor", "captureId", "finalCapture"] {
        if let Some(value) = v.get(field) {
            input[field] = value.clone();
        }
    }
    enqueue(
        a,
        h,
        &attempt_id,
        op,
        command
            .request_key
            .as_deref()
            .ok_or(bad("Request key required"))?,
        &input,
    )
    .await
}
/// Shared schema for generated apps, visual editors and explicit Flow Builder actions.
pub fn command_action() -> Value {
    json!({"name":"payment_command","description":"Queue provider payment operation","handler":"payment_command","public":false,"permission":"payments.manage","mcp":true,"flowAllowed":true,"inputSchema":{"type":"object","properties":{"operation":{"type":"string","enum":["capture","authorize","void","cancel","refund","reconcile"]},"attemptId":{"type":"string"},"event":{"type":"object"},"requestKey":{"type":"string"},"approve":{"type":"boolean"},"amountMinor":{"type":"integer"},"captureId":{"type":"string"},"finalCapture":{"type":"boolean"}},"required":["operation","requestKey","approve"],"additionalProperties":false}})
}
