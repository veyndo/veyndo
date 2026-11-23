//! Routes of typed configuration CRUD, rule preview and coupon edits; the marketing component owns their rules.
use super::*;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .merge(channel_preview::router())
        .merge(catalog::router())
        .merge(metadata::router())
        .secure_route("/api/automation", &[("GET", "read")], get(list))
        .secure_route(
            "/api/automation/executions",
            &[("GET", "read")],
            get(jobs::list),
        )
        .secure_route(
            "/api/automation/{kind}/{id}",
            &[("PUT", "read"), ("DELETE", "read")],
            axum::routing::put(save).delete(lifecycle::delete),
        )
        .secure_route(
            "/api/automation/{kind}/{id}/dependencies",
            &[("GET", "read")],
            get(lifecycle::dependencies),
        )
        .secure_route(
            "/api/automation/flows/{id}/validate",
            &[("POST", "automation.manage")],
            post(flow_policy::validate_draft),
        )
        .secure_route(
            "/api/automation/rules/preview",
            &[("POST", "read")],
            post(preview),
        )
        .route("/store-api/checkout/coupons", axum::routing::put(coupons))
        .secure_route(
            "/api/automation/rules/compare-history",
            &[("POST", "read")],
            post(rule_history::compare),
        )
}
pub(super) async fn list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    veyndo_marketing::service::configuration_writes::list(&a.core, &h)
        .await
        .map(Json)
}
pub(crate) async fn save(
    State(a): State<App>,
    h: RequestContext,
    Path((kind, id)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    veyndo_marketing::service::configuration_writes::save(
        &super::configuration_host::Configuration(&a),
        &h,
        (&kind, &id),
        &v,
    )
    .await
    .map(Json)
}
pub(super) async fn preview(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_marketing::service::rule_preview::preview(
            &a,
            &crate::cart_storage::Carts(&a),
            &h,
            &v,
        )
        .await?,
    ))
}
