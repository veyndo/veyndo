//! Digest-bound per-shop app secret rotation is handled in the apps component; this module registers its routes.
use super::*;
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/secrets",
            &[("GET", "apps.manage")],
            get(veyndo_app_platform::http::secrets::list),
        )
        .secure_route(
            "/api/apps/{id}/secrets/{kind}",
            &[("PUT", "apps.manage")],
            axum::routing::put(veyndo_app_platform::http::secrets::rotate),
        )
}
pub(super) use veyndo_app_platform::store::secrets::resolve;
