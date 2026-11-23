//! Reorder proposal routes; the intelligence component owns the supplier offers, the ceiling check and the checkout handover.
use super::*;
pub(crate) async fn propose(a: &App, h: &RequestContext) -> Result<Value> {
    veyndo_intelligence::service::supply_proposal::propose(
        &a.core,
        &crate::cart_storage::Carts(a),
        h,
    )
    .await
}
pub(crate) async fn approve(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::supply_proposal::approve(
        &a.core,
        &crate::cart_storage::Carts(a),
        h,
        v,
    )
    .await
}
async fn propose_http(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(propose(&a, &h).await?))
}
async fn approve_http(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(approve(&a, &h, &v).await?))
}
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/merchant/supply/proposal",
            &[("POST", "settings.read")],
            post(propose_http),
        )
        .secure_route(
            "/api/merchant/supply/approve",
            &[("POST", "settings.write")],
            post(approve_http),
        )
}
