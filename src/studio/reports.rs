//! Shared commerce reports live in the studio component; this module keeps the server registration and capability entries.
use crate::{App, RequestContext, Result, Router, SecureRoutes, Value, post};
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
