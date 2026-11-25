//! Payment status reads from one statement snapshot, and the merchant's view of a durable payment job.
use crate::domain::settlements::summary;
use crate::store::attempts::{self, Status};
use crate::store::jobs;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_identity::{merchant, permit};

/// The provider context fields the status projection shows.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StatusContext {
    checkout: Value,
    intent: Value,
    vault_status: Value,
    capabilities: Value,
}

/// One attempt with its latest job, and the sale when a cart is given.
async fn status_row(a: &State, t: &str, id: &str, cart: Option<&str>) -> Result<Status> {
    attempts::status(&a.db, t, id, cart)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Payment not found".into()))
}
/// The status projection of an attempt row.
fn payment_status(r: &Status, id: &str) -> Value {
    let p = &r.listed.attempt;
    let context: StatusContext = view(&p.context);
    let settled = summary(p);
    json!({"provider":p.provider,"capturedMinor":settled["capturedMinor"],"captureClosed":settled["captureClosed"],"checkout":context.checkout,"intent":context.intent,"job":r.job,"id":id,"state":p.state,"approvalUrl":r.listed.approval_url,"amountMinor":p.amount,"currency":p.currency,"currencyScale":p.currency_scale,"refundedMinor":p.refunded,"revision":r.listed.revision,"environment":p.environment,"savedPaymentMethodStatus":context.vault_status,"instructionsAvailable":context.capabilities.as_array().is_some_and(|cs|cs.iter().any(|c|c=="instructions")),"realMoneyCharged":p.environment=="live" && ["captured","partially_captured","captured_late","partially_refunded","refunded"].contains(&p.state.as_str())})
}
/// The status of one payment attempt.
pub async fn read(a: &State, t: &str, id: &str) -> Result<Value> {
    Ok(payment_status(&status_row(a, t, id, None).await?, id))
}
/// Caller must first admit the exact cashier/cart capability through the native POS owner.
pub async fn read_sale(a: &State, t: &str, id: &str, cart: &str) -> Result<Value> {
    let r = status_row(a, t, id, Some(cart)).await?;
    Ok(json!({"payment":payment_status(&r,id),"order":r.sale}))
}

/// Merchant polling observes the durable job instead of dispatching another financial command.
pub async fn job_status(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "payments.manage")?;
    let r = jobs::status(&a.db, &t, &id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Payment job not found".into()))?;
    Ok(Json(
        json!({"id":r.id,"state":r.state,"error":r.error,"payment":read(&a,&t,&r.attempt_id).await?}),
    ))
}
