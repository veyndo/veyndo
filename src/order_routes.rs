//! Merchant order read route; the orders component reads and projects the orders.
use crate::{App, auth, merchant};
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;

pub(crate) async fn orders(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "orders.read")?;
    auth::permit(&h, "customers.pii")?;
    Ok(Json(
        json!({"data":veyndo_orders::store::order_reads::recent(&a, &t).await?}),
    ))
}
