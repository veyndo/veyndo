//! Public claim batches live in the intelligence component; this module registers their storefront routes.
use crate::{App, Router, post};
use veyndo_intelligence::http::claims::{compile, intake};
pub(super) fn router() -> Router<App> {
    Router::new()
        .route("/store-api/intelligence/claims", post(intake))
        .route("/store-api/intelligence/claims/compile", post(compile))
}
