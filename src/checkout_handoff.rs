//! Single-use checkout transfer is owned by the checkout component; the server projects the redeemed cart.
use crate::{App, cart_json};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::header::CACHE_CONTROL;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
pub(crate) use veyndo_checkout::http::handoff::issue;
use veyndo_checkout::http::handoff::recover;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;

pub(crate) fn router() -> Router<App> {
    Router::new()
        .route("/checkout", get(crate::checkout_page::page))
        .route("/store-api/checkout/handoff", post(issue_http))
        .route("/store-api/checkout/handoff/consume", post(consume))
        .route("/store-api/checkout/handoff/recover", post(recover))
}

async fn issue_http(
    State(a): State<App>,
    h: RequestContext,
    body: Json<Value>,
) -> Result<impl IntoResponse> {
    Ok((
        [(CACHE_CONTROL, "no-store")],
        issue(State(a.core.clone()), h, body).await?,
    ))
}

async fn consume(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let c = veyndo_checkout::http::handoff::consume(&a, &h, &v).await?;
    Ok(Json(cart_json(&a, &c).await?))
}
