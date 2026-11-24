//! Relay only the browser's two Checkout session commands to a private service.
use crate::{App, Router, post};
use axum::extract::DefaultBodyLimit;
use veyndo_checkout::http::private_browser::{MAX_BODY, confirm, create};

pub(super) fn router() -> Router<App> {
    Router::new()
        .route("/store-api/checkout/independent/create", post(create))
        .route("/store-api/checkout/independent/confirm", post(confirm))
        .layer(DefaultBodyLimit::max(MAX_BODY))
}
