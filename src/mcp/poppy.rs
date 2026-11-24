//! Personal Agent Protocol discovery: a configured shop domain publishes poppy.json naming its guest MCP endpoint; no sign-in, session or UCP entry is claimed.
use crate::{
    App, Error, IntoResponse, Json, Response, State, StatusCode, header, runtime_config,
    shop_domains,
};
use crate::{Value, json};
pub(crate) const PATH: &str = "/.well-known/poppy.json";
/// The published path of a shop's document, or nothing when shops have no own domain on this installation.
pub(crate) fn published_url(shop: &str) -> Option<String> {
    let suffix = &runtime_config::get().shop_domain_suffix;
    (!suffix.is_empty()).then(|| format!("https://{shop}.{suffix}{PATH}"))
}
/// Poppy 1.0 requires `domain` to be the serving host, with its port when that is not 443, and HTTPS endpoints.
fn document(name: &str, domain: &str) -> Value {
    json!({"protocol_version":"1.0","organization":{"name":name,"domain":domain},
        "apis":[{"type":"mcp","url":format!("https://{domain}/mcp"),
            "description":"Guest shopping over MCP: search the catalog, read product details and signed facts, build a cart, get a signed offer and place an order through the shop's own checkout. No sign-in is offered; merchant tools need a merchant credential."}]})
}
/// Only the exact configured shop host is answered; legacy aliases, the API host and unbound hosts publish nothing.
fn serving_domain(host: &str, alias: &str, suffix: &str) -> Option<String> {
    let host = host.to_ascii_lowercase();
    let (name, port) = match host.rsplit_once(':') {
        Some((name, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            (name, Some(port))
        }
        _ => (host.as_str(), None),
    };
    if suffix.is_empty() || name != format!("{alias}.{suffix}") {
        return None;
    }
    Some(match port.filter(|port| *port != "443") {
        Some(port) => format!("{name}:{port}"),
        None => name.to_owned(),
    })
}
pub(crate) async fn discovery(State(a): State<App>, request: axum::extract::Request) -> Response {
    let missing = || {
        Error(
            StatusCode::NOT_FOUND,
            "No Poppy document on this host".into(),
        )
        .into_response()
    };
    let Some(shop) = request
        .extensions()
        .get::<shop_domains::HostShop>()
        .cloned()
    else {
        return missing();
    };
    let Some(domain) = serving_domain(
        header(request.headers(), "host").unwrap_or_default(),
        &shop.alias,
        &runtime_config::get().shop_domain_suffix,
    ) else {
        return missing();
    };
    // The display name is public shop identity; a failed read falls back to the alias instead of failing discovery.
    let name = veyndo::tenant_scope::scoped(
        veyndo::tenant_scope::Scope::Tenant(shop.access.tenant.clone()),
        sqlx::query_scalar::<_, String>("SELECT name FROM tenants WHERE id=$1")
            .bind(&shop.access.tenant)
            .fetch_optional(&a.db),
    )
    .await
    .ok()
    .flatten()
    .filter(|name| !name.trim().is_empty())
    .unwrap_or_else(|| shop.alias.clone());
    (
        // Caching follows the installation-wide response policy; agents on any origin may read the file.
        [(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")],
        Json(document(&name, &domain)),
    )
        .into_response()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_is_a_valid_minimal_poppy_file_without_sign_in_or_ucp_claims() {
        let doc = document("Nord Atelier", "nord.veyndo.ai");
        assert_eq!(doc["protocol_version"], "1.0");
        assert_eq!(
            doc["organization"],
            json!({"name":"Nord Atelier","domain":"nord.veyndo.ai"})
        );
        assert_eq!(doc["apis"][0]["type"], "mcp");
        assert_eq!(doc["apis"][0]["url"], "https://nord.veyndo.ai/mcp");
        assert!(
            doc["apis"][0]["description"]
                .as_str()
                .is_some_and(|d| !d.is_empty())
        );
        for absent in ["auth", "agent", "web", "tasks", "extensions"] {
            assert!(doc.get(absent).is_none(), "{absent}");
        }
    }
    #[test]
    fn only_the_exact_configured_shop_host_is_served_and_a_non_default_port_is_kept() {
        assert_eq!(
            serving_domain("Nord.Veyndo.AI", "nord", "veyndo.ai").as_deref(),
            Some("nord.veyndo.ai")
        );
        assert_eq!(
            serving_domain("nord.veyndo.ai:443", "nord", "veyndo.ai").as_deref(),
            Some("nord.veyndo.ai")
        );
        assert_eq!(
            serving_domain("nord.localhost:8787", "nord", "localhost").as_deref(),
            Some("nord.localhost:8787")
        );
        for (host, alias, suffix) in [
            ("nord.previous.test", "nord", "veyndo.ai"),
            ("other.veyndo.ai", "nord", "veyndo.ai"),
            ("nord.veyndo.ai.evil.test", "nord", "veyndo.ai"),
            ("nord.veyndo.ai", "nord", ""),
            ("127.0.0.1:8787", "nord", "veyndo.ai"),
        ] {
            assert!(serving_domain(host, alias, suffix).is_none(), "{host}");
        }
    }
}
