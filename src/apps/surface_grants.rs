//! Surface grants live in the apps component; this module registers their routes and keeps the short path for uploads.
use super::{Manifest, Surface, service_limits};
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::post;
use serde_json::Value;
pub(super) use veyndo_app_platform::domain::surface_grant::bind_input;
use veyndo_app_platform::http::surface_grants::{create, invoke};
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
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
