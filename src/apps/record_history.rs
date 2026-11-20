//! App record history lives in the apps component; this module registers its routes and keeps the capability path.
use super::*;
use veyndo_app_platform::http::record_history;
pub(crate) use veyndo_app_platform::http::record_history::schema;
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
