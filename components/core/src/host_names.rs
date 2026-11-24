//! Public host names: a syntactically valid domain, the shop alias under a configured suffix and origins of one family.
/// A lowercase DNS name with at least two labels.
pub fn domain(value: &str) -> bool {
    value.len() <= 253
        && value.contains('.')
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}
/// Configured former suffixes resolve through the same tenant/channel owner during cutover.
pub fn configured_alias<'a>(host: &'a str, canonical: &str, legacy: &str) -> Option<&'a str> {
    std::iter::once(canonical)
        .chain(legacy.split(','))
        .map(str::trim)
        .filter(|suffix| domain(suffix))
        .find_map(|suffix| host.strip_suffix(&format!(".{suffix}")))
}
/// Keep only the configured public origin's exact prefix on explicitly admitted former domains.
pub fn same_origin_family(
    actual: &reqwest::Url,
    expected: &reqwest::Url,
    canonical: &str,
    legacy: &str,
) -> bool {
    if actual.origin() == expected.origin() {
        return true;
    }
    if actual.scheme() != "https"
        || expected.scheme() != "https"
        || actual.port().is_some()
        || expected.port().is_some()
        || !domain(canonical)
    {
        return false;
    }
    let expected_host = expected.host_str().unwrap_or_default();
    let actual_host = actual.host_str().unwrap_or_default();
    if expected_host == canonical {
        return legacy
            .split(',')
            .map(str::trim)
            .any(|old| domain(old) && actual_host == old);
    }
    configured_alias(expected_host, canonical, "")
        .is_some_and(|prefix| configured_alias(actual_host, "", legacy) == Some(prefix))
}
/// Canonical links are server-owned; independent admin login always uses the configured Studio origin.
pub fn links(shop: &str) -> serde_json::Value {
    let origin =
        std::env::var("COMMERCE_PUBLIC_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:8787".into());
    let storefront = std::env::var("SHOP_DOMAIN_SUFFIX")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|suffix| format!("https://{shop}.{suffix}/"))
        .unwrap_or_else(|| format!("{}/?shop={shop}", origin.trim_end_matches('/')));
    serde_json::json!({"storefrontUrl":storefront,"studioUrl":format!("{}/?shop={shop}#merchant",origin.trim_end_matches('/'))})
}

/// An operator frontend origin: HTTPS or loopback HTTP, no credentials, path, query or fragment.
pub fn valid_origin(s: &str) -> bool {
    reqwest::Url::parse(s).is_ok_and(|u| {
        u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && u.path() == "/"
            && (u.scheme() == "https"
                || u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1" | "localhost")))
    })
}

/// Paths a hosted frontend may serve: never Core APIs, Store API, UCP, MCP, health or internal services.
pub fn public_path(path: &str) -> bool {
    if path.starts_with("/channel-preview/")
        || path.starts_with("/api/") && !public_api_path(path)
        || path.starts_with("/store-api/")
        || path.starts_with("/ucp/")
        || path == "/mcp"
        || path == "/health"
        || path.starts_with("/.well-known/")
    {
        return false;
    }
    if path.starts_with("/experience-api/") {
        return path == "/experience-api/context" || path.starts_with("/experience-api/shops/");
    }
    !path.starts_with("/experience-internal/")
}
/// Reserved public frontend API namespace; never admits encoded or ambiguous paths.
pub fn public_api_path(path: &str) -> bool {
    path.strip_prefix("/api/v1/").is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            })
    })
}
#[cfg(test)]
mod tests {
    use super::{configured_alias, public_path, same_origin_family, valid_origin};
    #[test]
    fn aliases_keep_exact_suffix_boundaries_and_existing_prefix_validation() {
        for host in ["shop.veyndo.ai", "shop.previous.test"] {
            assert_eq!(
                configured_alias(host, "veyndo.ai", "previous.test"),
                Some("shop")
            );
        }
        for host in [
            "shop.previous.test.evil.test",
            "shop.evilprevious.test",
            "previous.test",
        ] {
            assert_eq!(configured_alias(host, "veyndo.ai", "previous.test"), None);
        }
        // The existing resolver rejects a multi-label or invalid tenant instead of falling back.
        assert_eq!(
            configured_alias("nested.shop.previous.test", "veyndo.ai", "previous.test"),
            Some("nested.shop")
        );
        assert_eq!(
            configured_alias("shop.previous.test", "veyndo.ai", "https://previous.test"),
            None
        );
    }
    #[test]
    fn legacy_public_origins_keep_prefix_https_and_port_constraints() {
        let expected = reqwest::Url::parse("https://app.veyndo.ai").unwrap();
        for (value, allowed) in [
            ("https://app.previous.test", true),
            ("https://other.previous.test", false),
            ("https://app.previous.test.evil.test", false),
            ("http://app.previous.test", false),
            ("https://app.previous.test:444", false),
        ] {
            assert_eq!(
                same_origin_family(
                    &reqwest::Url::parse(value).unwrap(),
                    &expected,
                    "veyndo.ai",
                    "previous.test"
                ),
                allowed,
                "{value}"
            );
        }
    }
    #[test]
    fn operator_origins_are_bare_https_or_loopback() {
        assert!(valid_origin("https://frontends.example/"));
        assert!(valid_origin("http://127.0.0.1:8790/"));
        for bad in [
            "http://frontends.example/",
            "https://u:p@x.example/",
            "https://x.example/path",
        ] {
            assert!(!valid_origin(bad), "{bad}");
        }
    }
    #[test]
    fn hosted_frontends_never_serve_core_paths() {
        for p in [
            "/store-api/product",
            "/ucp/v1",
            "/mcp",
            "/api/v1/../x",
            "/experience-internal/a",
        ] {
            assert!(!public_path(p), "{p}");
        }
        for p in ["/", "/api/v1/products/p-1", "/experience-api/shops/demo"] {
            assert!(public_path(p), "{p}");
        }
    }
}
