//! Incoming app webhooks are handled in the apps component; this module registers their public route.
use super::*;
pub(super) fn router() -> Router<App> {
    Router::new().route(
        "/webhooks/apps/{tenant}/{app}/{webhook}",
        post(veyndo_app_platform::http::webhooks::receive),
    )
}
