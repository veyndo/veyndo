//! Builds the pinned adapter request of one payment operation; the gateway owns transport and admission.
use crate::domain::attempt::Attempt;
use crate::domain::return_urls;
use crate::ports::Gateway;
use crate::store::order_ledger::order_document;
use serde_json::{Value, json};
use veyndo_core::{Result, State};

pub async fn execute(
    gateway: &dyn Gateway,
    a: &State,
    p: &Attempt,
    op: &str,
    key: &str,
    input: &Value,
) -> Result<Value> {
    crate::service::funding::dispatch_check(a, p, op).await?;
    let mut order: Value = if ["inspect", "instructions", "vault_revoke"].contains(&op) {
        Value::Null
    } else {
        order_document(&a.db, &p.tenant, &p.order).await?
    };
    if let Some(c) = order["cart"].as_object_mut() {
        c.remove("token");
    }
    let ctx = p.ctx();
    let (return_url, cancel_url) = if let Some(origin) = ctx.return_origin.as_str() {
        return_urls::build(
            origin,
            &p.tenant,
            ctx.channel.as_str().unwrap_or("default"),
            &p.id,
            p.environment == "live",
        )?
    } else {
        return_urls::urls(
            &p.tenant,
            ctx.channel.as_str().unwrap_or("default"),
            &p.id,
            p.environment == "live",
        )?
    };
    let mut context = p.context.clone();
    if let Some(context) = context.as_object_mut() {
        context.remove("providerRefunds");
        context.remove("captureReceipts");
    }
    let capture_id = input
        .get("captureId")
        .cloned()
        .unwrap_or_else(|| json!(p.capture));
    let mut body = json!({"apiVersion":"1","provider":p.provider,"adapterVersion":p.adapter_version,"tenant":p.tenant,"attemptId":p.id,"orderId":p.order,"environment":p.environment,"accountRef":ctx.account_ref,"method":ctx.method,"intent":ctx.intent,"amountMinor":p.amount,"currency":p.currency,"currencyScale":p.currency_scale,"reference":p.provider_order,"captureId":capture_id,"context":context,"operation":op,"requestKey":key,"input":input,"order":order,"returnUrl":return_url,"cancelUrl":cancel_url});
    if let Some(funding) = p.context.get("funding") {
        body["funding"] = funding.clone();
    }
    gateway
        .call(
            &p.tenant,
            &p.provider,
            &p.adapter_version,
            &p.environment,
            "payments/execute",
            &body,
        )
        .await
}
