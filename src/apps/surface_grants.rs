//! Surface grants live in the apps component; this module registers their routes and keeps the short path for uploads.
use super::{Manifest, Surface, service_limits};
use crate::{App, RequestContext, Result, Router, SecureRoutes, Value, post};
pub(super) use veyndo_app_platform::domain::surface_grant::bind_input;
use veyndo_app_platform::http::surface_grants::{create, invoke};
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/surfaces/{surface}/grant",
            &[("POST", "read")],
            post(create),
        )
        .route(
            "/store-api/apps/{id}/surfaces/{surface}/grant",
            post(create),
        )
        .secure_route(
            "/api/apps/{id}/surfaces/{surface}/actions/{action}",
            &[("POST", "read")],
            post(invoke),
        )
        .route(
            "/store-api/apps/{id}/surfaces/{surface}/actions/{action}",
            post(invoke),
        )
}
/// Recheck current actor, immutable digest, role and object ownership for uploads.
pub(super) async fn load(
    a: &App,
    h: &RequestContext,
    id: &str,
    surface: &str,
    v: &Value,
) -> Result<(Manifest, Surface, Value, Value)> {
    veyndo_app_platform::store::surface_grants::load(
        &service_limits::platform(a),
        h,
        id,
        surface,
        v,
    )
    .await
}
