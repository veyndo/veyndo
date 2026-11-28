//! Anonymous cross-origin reads of the public Store API, limited to the operations in contracts/browser-reads.json.
//!
//! The handbook's Try-it panels and browser storefronts on another origin may read public catalog data.
//! Only listed method/path templates qualify, never with a bearer credential, cart/buyer token or cookie,
//! and a preflight may only ask for the listed headers. Credentials are never allowed (`*` origin), so a
//! browser cannot attach cookies to a readable response. POST /mcp keeps its own origin policy.
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::LazyLock;

struct Policy {
    headers: Vec<String>,
    reads: Vec<(Method, Vec<String>)>,
}

static POLICY: LazyLock<Policy> =
    LazyLock::new(|| parse(include_str!("../contracts/browser-reads.json")));

/// The wire shape of contracts/browser-reads.json; `body` and `description` serve the handbook only.
#[derive(serde::Deserialize)]
struct Contract {
    headers: Vec<String>,
    reads: Vec<Read>,
}

#[derive(serde::Deserialize)]
struct Read {
    method: String,
    path: String,
}

fn parse(text: &str) -> Policy {
    let contract: Contract = serde_json::from_str(text).expect("browser read contract");
    let headers = contract
        .headers
        .iter()
        .map(|h| h.to_ascii_lowercase())
        .collect();
    let reads = contract
        .reads
        .iter()
        .map(|read| {
            let method = Method::from_bytes(read.method.as_bytes()).expect("HTTP method");
            assert!(
                method == Method::GET || method == Method::POST,
                "browser reads are GET or POST"
            );
            (method, segments(&read.path).map(str::to_owned).collect())
        })
        .collect();
    Policy { headers, reads }
}

fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.trim_start_matches('/').split('/')
}

/// A `{name}` template segment matches exactly one non-empty path segment.
fn matches(template: &[String], path: &str) -> bool {
    let parts: Vec<&str> = segments(path).collect();
    template.len() == parts.len()
        && template.iter().zip(&parts).all(|(t, p)| {
            if t.starts_with('{') {
                !p.is_empty()
            } else {
                t == p
            }
        })
}

fn admitted(policy: &Policy, method: &Method, path: &str) -> bool {
    policy
        .reads
        .iter()
        .any(|(m, template)| m == method && matches(template, path))
}

fn credentialed(headers: &HeaderMap) -> bool {
    headers.contains_key(header::AUTHORIZATION)
        || headers.contains_key(header::COOKIE)
        || headers.contains_key("sw-context-token")
}

fn requested_headers_allowed(policy: &Policy, headers: &HeaderMap) -> bool {
    let Some(requested) = headers.get(header::ACCESS_CONTROL_REQUEST_HEADERS) else {
        return true;
    };
    let Ok(requested) = requested.to_str() else {
        return false;
    };
    requested
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .all(|h| policy.headers.contains(&h))
}

fn preflight(policy: &Policy, method: &Method) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_str(method.as_str()).expect("method header"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_str(&policy.headers.join(", ")).expect("header list"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    response
}

pub(crate) async fn apply(request: Request, next: Next) -> Response {
    if !request.headers().contains_key(header::ORIGIN) {
        return next.run(request).await;
    }
    let policy = &*POLICY;
    let path = request.uri().path().to_owned();
    if request.method() == Method::OPTIONS {
        let asked = request
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_METHOD)
            .and_then(|v| Method::from_bytes(v.as_bytes()).ok());
        if let Some(method) = asked.filter(|m| admitted(policy, m, &path)) {
            if requested_headers_allowed(policy, request.headers()) {
                return preflight(policy, &method);
            }
            // A listed read asked for a credential or another header: refuse without CORS headers.
            return StatusCode::FORBIDDEN.into_response();
        }
        return next.run(request).await;
    }
    let open = !credentialed(request.headers()) && admitted(policy, request.method(), &path);
    let mut response = next.run(request).await;
    if open {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> &'static Policy {
        &POLICY
    }

    #[test]
    fn contract_parses_and_lists_only_reads() {
        assert!(!policy().reads.is_empty());
        assert!(policy().headers.iter().any(|h| h == "x-tenant"));
        assert!(!policy().headers.iter().any(|h| h == "authorization"));
        assert!(!policy().headers.iter().any(|h| h == "sw-context-token"));
    }

    #[test]
    fn templates_match_one_segment_per_parameter() {
        let p = policy();
        assert!(admitted(p, &Method::POST, "/store-api/product"));
        assert!(admitted(p, &Method::GET, "/store-api/product/coat-m"));
        assert!(!admitted(p, &Method::GET, "/store-api/product/"));
        assert!(!admitted(p, &Method::GET, "/store-api/product/a/b/c"));
        assert!(!admitted(p, &Method::DELETE, "/store-api/product/coat-m"));
        assert!(!admitted(p, &Method::GET, "/store-api/account/orders"));
        assert!(!admitted(p, &Method::POST, "/store-api/checkout/order"));
        assert!(!admitted(p, &Method::POST, "/mcp"));
    }

    #[test]
    fn credentials_and_unlisted_headers_are_refused() {
        let p = policy();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("content-type, x-tenant"),
        );
        assert!(requested_headers_allowed(p, &headers));
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("x-tenant, authorization"),
        );
        assert!(!requested_headers_allowed(p, &headers));
        headers.insert(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("sw-context-token"),
        );
        assert!(!requested_headers_allowed(p, &headers));
        let mut sent = HeaderMap::new();
        assert!(!credentialed(&sent));
        sent.insert(header::COOKIE, HeaderValue::from_static("a=b"));
        assert!(credentialed(&sent));
    }
}
