//! Merchant channel previews are owned by the marketing component; the server registers their routes.
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::get;
use axum::routing::post;
use veyndo_marketing::http::channel_preview::{create, end, redeem};
pub(crate) use veyndo_marketing::service::channel_preview::cookie;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/automation/channels/{id}/preview",
            &[("POST", "channels.read")],
            post(create),
        )
        .route("/channel-preview/{ticket}", get(redeem))
        .route("/channel-preview/end", post(end))
}
