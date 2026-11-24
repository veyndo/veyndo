//! Generic operator-allowlisted frontend mounts. Host scope is derived from storage, never client headers.
use crate::{App, SecureRoutes};
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::{extract::Request, middleware::Next};
use std::env;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::shop_scope::header;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/settings/frontends",
            &[("GET", "settings.read"), ("PUT", "settings.write")],
            get(super::frontend_bindings::list).put(super::frontend_bindings::bind),
        )
        .secure_route(
            "/api/settings/frontends/{alias}",
            &[("DELETE", "settings.write")],
            axum::routing::delete(super::frontend_bindings::remove),
        )
}
pub(super) use veyndo_core::host_names::valid_origin;
pub(crate) use veyndo_core::host_names::{public_api_path, public_path};
/// Only an operator-bound hostname may delegate an otherwise unregistered route.
/// Registered Core routes always keep their own permission and handler.
pub(crate) fn delegated_api(request: &Request) -> bool {
    public_api_path(request.uri().path())
        && request
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .is_none()
        && request
            .extensions()
            .get::<super::HostShop>()
            .is_some_and(|h| h.access.mount.is_some())
}
pub(crate) async fn serve(State(a): State<App>, request: Request, next: Next) -> Response {
    let Some(host) = request.extensions().get::<super::HostShop>().cloned() else {
        return next.run(request).await;
    };
    if !public_path(request.uri().path())
        || request.uri().path().starts_with("/api/") && !delegated_api(&request)
    {
        return next.run(request).await;
    }
    let Some(mount) = host.access.mount.as_ref() else {
        return next.run(request).await;
    };
    if host.access.status.as_deref() != Some("active") {
        return Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Shop or sales channel is unavailable".into(),
        )
        .into_response();
    }
    match proxy(&a, request, &host.alias, &host.access, mount).await {
        Ok(r) => r,
        Err(e) => e.into_response(),
    }
}
async fn proxy(
    a: &App,
    request: Request,
    alias: &str,
    access: &crate::performance::access_snapshot::AccessSnapshot,
    mount: &veyndo_core::access::FrontendMount,
) -> Result<Response> {
    let origin = &mount.origin;
    if !valid_origin(origin)
        || env::var("HOSTED_FRONTEND_ORIGIN").ok().as_deref() != Some(origin.as_str())
    {
        return Err(bad("Frontend destination unavailable"));
    }
    let key = env::var("HOSTED_FRONTEND_KEY")
        .ok()
        .filter(|k| k.len() >= 64)
        .ok_or(bad("Frontend gateway not configured"))?;
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 8_000_000)
        .await
        .map_err(|_| bad("Frontend body exceeds limit"))?;
    let path = parts
        .uri
        .path_and_query()
        .map(|s| s.as_str())
        .unwrap_or("/");
    let target = format!("{}{}", origin.trim_end_matches('/'), path);
    let mut req = a
        .http
        .request(
            reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
                .map_err(|_| bad("Invalid method"))?,
            target,
        )
        .body(bytes)
        .header("x-frontend-key", key)
        .header("x-frontend-alias", &mount.experience_alias)
        .header("x-frontend-host", alias)
        .header("x-frontend-tenant", &access.tenant)
        .header("x-frontend-channel", &access.channel_id);
    for key in [
        "content-type",
        "accept",
        "last-event-id",
        "range",
        "if-none-match",
        "x-locale",
        "x-cart-token",
        "x-customer-token",
        "idempotency-key",
        "origin",
    ] {
        if let Some(value) = parts.headers.get(key) {
            req = req.header(key, value);
        }
    }
    if let Some(preview) = header(&parts.headers, "x-channel-preview")
        .map(str::to_owned)
        .or_else(|| crate::marketing::channel_preview::cookie(&parts.headers))
    {
        req = req.header("x-channel-preview", preview);
    }
    if let Some(cookie) = veyndo_shop::http::frontend_transport::request_cookie(&parts.headers) {
        req = req.header("cookie", cookie);
    }
    let r = req.send().await.map_err(|_| {
        Error(
            StatusCode::BAD_GATEWAY,
            "Frontend service unavailable".into(),
        )
    })?;
    Ok(veyndo_shop::http::frontend_transport::response(
        r,
        &crate::runtime_config::get().frame_ancestors,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mount_never_captures_core_or_internal_services() {
        for p in [
            "/api/auth/login",
            "/api/platform/shops",
            "/store-api/checkout/order",
            "/mcp",
            "/.well-known/ucp",
            "/experience-internal/credentials",
            "/experience-api/auth/login",
            "/api/v1/../platform/shops",
            "/api/v1/%2e%2e/merchant",
            "/api/v1//commerce.json",
            "/api/v1/",
        ] {
            assert!(!public_path(p));
        }
        for p in [
            "/",
            "/assets/app.js",
            "/experience-api/shops/demo/commerce/checkout/cart",
            "/api/v1/commerce.json",
            "/api/v1/shopper-session.json",
            "/api/v1/products/product-123",
        ] {
            assert!(public_path(p));
        }
        assert!(valid_origin("https://experience.example.test"));
        assert!(!valid_origin("https://key@example.test"));
        assert!(!valid_origin("http://169.254.169.254"));
    }
}
