//! Merchant memory endpoints and revision-bound experiment/dismissal decisions.
use super::*;
pub(crate) fn cognition_router() -> Router<App> {
    Router::new()
        .merge(contracts::router())
        .merge(behavior::router())
        .merge(preferences::router())
        .merge(signed::router())
        .merge(signed_offer::router())
        .merge(stream::router())
        .route(
            "/store-api/intelligence/recommendations/{id}",
            get(recommendations),
        )
        .secure_route(
            "/api/intelligence",
            &[("GET", "knowledge.read")],
            get(veyndo_intelligence::http::memory::memory),
        )
        .secure_route(
            "/api/intelligence/hypotheses/{id}",
            &[("PUT", "catalog.write")],
            axum::routing::put(veyndo_intelligence::http::memory::decide),
        )
}
