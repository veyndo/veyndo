//! Shared commerce reports live in the studio component; this module keeps the server registration and capability entries.
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::post;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) mod config;
pub(crate) use config::schema;
pub(crate) use veyndo_studio::service::reports::visible;
pub(crate) fn router() -> Router<App> {
    Router::new().secure_route(
        "/api/merchant/reports",
        &[("POST", "read")],
        post(veyndo_studio::http::reports::report),
    )
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_studio::service::reports::invoke(&a.core, h, v).await
}
