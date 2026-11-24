//! Customer-owned saved instruments expose only redacted metadata; immediate revocation and provider cleanup share payment jobs.
use crate::accounts;
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
pub(crate) async fn identity(a: &App, h: &RequestContext) -> Result<(String, String)> {
    let (t, email) = accounts::identity(a, h).await?;
    let id = veyndo_customers::store::customers::active_id(a, &t, &email).await?;
    Ok((t, id))
}
/// A saved payment instrument operation: the instrument, its request key and approval.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InstrumentArgs {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    request_key: Option<String>,
}

pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let input: InstrumentArgs = view(v);
    if name == "checkout.payment.methods" {
        list(a, h).await
    } else {
        if input.approve != Some(true) {
            return Err(bad("Approve saved payment revocation"));
        }
        revoke(
            a,
            h,
            input.id.as_deref().ok_or(bad("id required"))?,
            input
                .request_key
                .as_deref()
                .ok_or(bad("requestKey required"))?,
        )
        .await
    }
}
pub(crate) async fn list(a: &App, h: &RequestContext) -> Result<Value> {
    let (t, customer) = identity(a, h).await?;
    veyndo_payments::store::saved_instruments::list(a, &t, &customer).await
}
pub(crate) async fn revoke(a: &App, h: &RequestContext, id: &str, key: &str) -> Result<Value> {
    let (t, customer) = identity(a, h).await?;
    veyndo_payments::store::saved_instruments::revoke(a, &t, &customer, id, key).await
}
pub(crate) async fn route_list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(list(&a, &h).await?))
}
pub(crate) async fn route_revoke(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        revoke(
            &a,
            &h,
            &id,
            v["requestKey"]
                .as_str()
                .or_else(|| header(&h, "idempotency-key"))
                .ok_or(bad("requestKey required"))?,
        )
        .await?,
    ))
}
