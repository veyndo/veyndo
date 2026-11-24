//! App-owned UI surfaces and namespaced HTTP routes reuse the authorized action gateway.
use crate::{App, Json, RequestContext, Response, Result, Router, SecureRoutes, State, Value, get};
use veyndo_app_platform::http::app_routes::{read_route, write_route};
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/surfaces",
            &[("GET", "catalog.read")],
            get(admin_surfaces),
        )
        .route("/store-api/apps/surfaces", get(public_surfaces))
        .secure_route(
            "/api/apps/{id}/http/{route}",
            &[("GET", "read"), ("POST", "read")],
            get(read_route).post(write_route),
        )
        .route(
            "/store-api/apps/{id}/http/{route}",
            get(read_route).post(write_route),
        )
        .layer(axum::middleware::from_fn(no_store))
}
async fn no_store(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
pub(crate) use super::surface_registry::registry;
pub(crate) use veyndo_app_platform::domain::surfaces::validate_contract;
async fn admin_surfaces(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(registry(&a, &h, false).await?))
}
async fn public_surfaces(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(registry(&a, &h, true).await?))
}
