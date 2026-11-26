//! App record history lives in the apps component; this module registers its routes and keeps the capability path.
use super::service_limits;
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
use veyndo_app_platform::http::record_history;
pub(crate) use veyndo_app_platform::http::record_history::schema;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{app}/entities/{entity}/{record}/history",
            &[("GET", "read")],
            get(record_history::list),
        )
        .secure_route(
            "/api/apps/{app}/entities/{entity}/{record}/history/{revision}/restore",
            &[("POST", "read")],
            post(record_history::restore),
        )
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    record_history::invoke(&service_limits::platform(a), h, name, v).await
}
