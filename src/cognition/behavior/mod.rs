//! Native behavior analytics projects the existing event ledger into private shop knowledge.
use crate::{App, Json, RequestContext, Result, Router, SecureRoutes, State, Value, post};
use veyndo_intelligence::http::behavior_capture as capture;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .route(
            "/store-api/behavior/events",
            post(capture::capture)
                .get(capture::export)
                .delete(capture::forget),
        )
        .secure_route(
            "/api/knowledge/behavior",
            &[("POST", "knowledge.read")],
            post(report_api),
        )
        .secure_route(
            "/api/knowledge/behavior/journey",
            &[("POST", "customers.pii")],
            post(journey_api),
        )
}
async fn report_api(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "knowledge.behavior", &v).await?))
}
async fn journey_api(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "knowledge.behavior.journey", &v).await?,
    ))
}
pub(crate) use veyndo_intelligence::service::behavior::{invoke, schema};
pub(crate) use veyndo_intelligence::store::behavior::retire;
