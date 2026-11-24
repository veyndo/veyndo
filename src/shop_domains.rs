//! Resolve configured shop subdomains before authentication; reject unknown hosts and conflicting scopes.
use crate::{App, api_host, runtime_config};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::{extract::Request, middleware::Next};
use std::env;
use veyndo_core::Error;
use veyndo_core::bad;
use veyndo_core::shop_scope::header;
use veyndo_core::validate_tenant;
mod frontend_bindings;
pub(crate) mod frontend_editor;
pub(crate) mod frontends;
pub(crate) use veyndo_core::host_names::configured_alias;
pub(crate) use veyndo_core::request_context::HostShop;
pub(crate) async fn resolve(State(a): State<App>, mut request: Request, next: Next) -> Response {
    let domain = &runtime_config::get().shop_domain_suffix;
    let legacy = env::var("LEGACY_DOMAINS").unwrap_or_default();
    let host = header(request.headers(), "host")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let host = host.split(':').next().unwrap_or_default();
    if api_host(host, domain) {
        return next.run(request).await;
    }
    if let Some(shop) = configured_alias(host, domain, &legacy)
        && !["app", "www", "admin", "experience"].contains(&shop)
    {
        if validate_tenant(shop).is_err()
            || shop.contains('.')
            || ["api", "admin", "mail", "platform", "experience"].contains(&shop)
        {
            return Error(StatusCode::NOT_FOUND, "Unknown shop".into()).into_response();
        }
        let channel = header(request.headers(), "sw-sales-channel-id").unwrap_or("default");
        let access = veyndo::tenant_scope::scoped(
            veyndo::tenant_scope::Scope::System,
            crate::performance::access_snapshot::load(&a, Some(shop), shop, channel),
        )
        .await;
        let access = match access {
            Ok(access) => access,
            Err(_) => {
                return Error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Domain resolution unavailable".into(),
                )
                .into_response();
            }
        };
        let scope = &access.tenant;
        if header(request.headers(), "x-tenant").is_some_and(|t| t != scope) {
            return bad("Shop hostname and request scope disagree").into_response();
        }
        if !access.exists {
            return Error(StatusCode::NOT_FOUND, "Unknown shop".into()).into_response();
        }
        if access.mount.is_some() {
            let channel = &access.channel_id;
            if header(request.headers(), "sw-sales-channel-id").is_some_and(|id| id != channel) {
                return bad("Shop hostname and channel disagree").into_response();
            }
            request
                .headers_mut()
                .insert("sw-sales-channel-id", channel.parse().unwrap());
        }
        request
            .headers_mut()
            .insert("x-tenant", scope.parse().unwrap());
        request.extensions_mut().insert(HostShop {
            alias: shop.into(),
            access,
        });
    }
    next.run(request).await
}

pub(crate) use veyndo_core::host_names::links;
