//! Shared read-context caching with authoritative versions; mutations and checkout locks stay outside memoization.
use crate::commerce;
use crate::foundation::App;
use axum::http::StatusCode;
use axum::response::Response;
use axum::{extract::Request, middleware::Next};
use std::sync::Arc;
use veyndo_core::Result;
pub(crate) mod access_snapshot;
pub(crate) use veyndo_core::database::lease as cluster_lease;
pub(crate) mod delivery;
pub(crate) use veyndo_core::admission::{Admission, AiStreamAdmission, reserve_ai_attempt};
mod invalidation;
pub(crate) use invalidation::start as start_invalidations;
pub(crate) use veyndo_core::database::authority::connect as connect_database;
pub(crate) use veyndo_core::database::row_security::verify as verify_row_security;

pub(crate) use veyndo_shop::service::read_cache::{LanguageRow, read_scope, reads};
/// The settings of a sales channel through the shop read cache.
pub(crate) async fn settings(
    a: &App,
    tenant: &str,
    channel: &str,
) -> Result<(Arc<commerce::Settings>, i64)> {
    veyndo_shop::service::read_cache::settings(&a.core, tenant, channel).await
}
/// The global language registry through the shop read cache.
pub(crate) async fn languages(a: &App) -> Result<Arc<Vec<LanguageRow>>> {
    veyndo_shop::service::read_cache::languages(&a.core).await
}
pub(crate) async fn memoize(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let read = request.method() == axum::http::Method::GET
        || request.method() == axum::http::Method::POST
            && [
                "/store-api/product",
                "/api/search/product",
                "/api/search/order",
                "/api/merchant/quote",
            ]
            .contains(&request.uri().path());
    let mut response = if read {
        read_scope(next.run(request)).await
    } else {
        next.run(request).await
    };
    let policy = if response.status() == StatusCode::OK && hashed_asset(&path) {
        Some("public, max-age=31536000, immutable")
    } else if path == "/" {
        Some("no-cache")
    } else if path.starts_with("/api/")
        || path.starts_with("/store-api/")
        || path == "/mcp"
        || path.starts_with("/ucp/")
    {
        Some("no-store")
    } else {
        None
    };
    // Explicit image/app policies retain their existing scoped behavior.
    if let Some(policy) = policy
        && !response.headers().contains_key("cache-control")
    {
        response
            .headers_mut()
            .insert("cache-control", policy.parse().unwrap());
    }
    response
}
fn hashed_asset(path: &str) -> bool {
    let Some(file) = path.strip_prefix("/assets/") else {
        return false;
    };
    if file.contains('/') {
        return false;
    }
    let Some((stem, ext)) = file.rsplit_once('.') else {
        return false;
    };
    let bytes = stem.as_bytes();
    if bytes.len() < 10 || bytes[bytes.len() - 9] != b'-' {
        return false;
    }
    ["js", "css"].contains(&ext)
        && bytes[bytes.len() - 8..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
}
#[cfg(test)]
mod tests {
    #[test]
    fn immutable_policy_only_matches_fingerprinted_frontend_assets() {
        assert!(super::hashed_asset("/assets/index-123Ab_cd.js"));
        assert!(super::hashed_asset("/assets/index-12-Ab_cd.js"));
        for path in [
            "/api/private-123Ab_cd.js",
            "/assets/index.js",
            "/assets/sub/index-123Ab_cd.js",
            "/assets/index-123Ab_cd.svg",
        ] {
            assert!(!super::hashed_asset(path));
        }
    }
}
