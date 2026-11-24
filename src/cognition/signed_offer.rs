//! Signed cart offer transport; the intelligence component signs and checks offers through the cart port.
use crate::cart_storage::Carts;
use crate::{App, Json, RequestContext, Result, Router, State, Value, get, post};
use veyndo_intelligence::service::signed_offer::{issue, verify};
pub(crate) async fn issue_offer(a: &App, h: &RequestContext) -> Result<Value> {
    issue(&Carts(a), h).await
}
pub(crate) async fn verify_offer(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    verify(&Carts(a), h, v).await
}
async fn issue_http(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(issue(&Carts(&a), &h).await?))
}
async fn verify_http(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(verify(&Carts(&a), &h, &v).await?))
}
pub(crate) fn router() -> Router<App> {
    Router::new()
        .route("/store-api/checkout/cart/offer", get(issue_http))
        .route("/store-api/checkout/offer/verify", post(verify_http))
}
