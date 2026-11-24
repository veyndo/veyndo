//! Incoming app webhooks are handled in the apps component; this module registers their public route.
use crate::foundation::App;
use axum::Router;
use axum::routing::post;
pub(super) fn router() -> Router<App> {
    Router::new().route(
        "/webhooks/apps/{tenant}/{app}/{webhook}",
        post(veyndo_app_platform::http::webhooks::receive),
    )
}
