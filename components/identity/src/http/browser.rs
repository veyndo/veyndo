//! Browser transport for the existing personal-session ledger: host-only HttpOnly cookies, exact origins and CSRF.
use axum::Json;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use std::env;
use veyndo_core::{Error, StatusCode, bad, hash, process_config};

const MARKER: &str = "browser-session";
fn token(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn csrf(session: &str) -> String {
    hash(&format!("veyndo-browser-csrf:v1:{session}"))
}
pub fn equal(value: &str, expected: &str) -> bool {
    let mut a = Hmac::<Sha256>::new_from_slice(b"veyndo-browser-csrf-comparison:v1").unwrap();
    a.update(expected.as_bytes());
    let signature = a.finalize().into_bytes();
    let mut b = Hmac::<Sha256>::new_from_slice(b"veyndo-browser-csrf-comparison:v1").unwrap();
    b.update(value.as_bytes());
    b.verify_slice(&signature).is_ok()
}
fn cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let mut values = headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|v| v.trim().split_once('='))
        .filter(|(k, _)| *k == name)
        .map(|(_, v)| v);
    let value = values.next()?;
    (values.next().is_none() && token(value)).then(|| value.to_owned())
}
fn names(secure: bool) -> (&'static str, &'static str) {
    if secure {
        ("__Host-veyndo-session", "__Host-veyndo-csrf")
    } else {
        ("veyndo-session", "veyndo-csrf")
    }
}
pub fn origin(request: &Request) -> Option<(String, bool)> {
    let host = request.headers().get("host")?.to_str().ok()?;
    let configured = env::var("COMMERCE_PUBLIC_ORIGIN")
        .ok()
        .and_then(|s| reqwest::Url::parse(&s).ok());
    if let Some(url) = configured
        && url
            .origin()
            .ascii_serialization()
            .strip_prefix(&format!("{}://", url.scheme()))
            == Some(host)
        && url.scheme() == "https"
    {
        return Some((url.origin().ascii_serialization(), true));
    }
    // Insecure cookies are restricted to explicit local development hosts.
    if !process_config::get().strict {
        let url = reqwest::Url::parse(&format!("http://{host}")).ok()?;
        if matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
            && url.username().is_empty()
            && url.password().is_none()
        {
            return Some((url.origin().ascii_serialization(), false));
        }
    }
    None
}
fn same_origin(request: &Request, expected: &str) -> bool {
    if let Some(value) = request.headers().get("origin") {
        return value.to_str().ok() == Some(expected);
    }
    matches!(request.method().as_str(), "GET" | "HEAD")
        && request
            .headers()
            .get("sec-fetch-site")
            .and_then(|v| v.to_str().ok())
            == Some("same-origin")
}
fn set_cookies(response: &mut Response, credential: Option<&str>, secure: bool) {
    let (session_name, csrf_name) = names(secure);
    let flags = if secure { "; Secure" } else { "" };
    let age = if credential.is_some() { 43200 } else { 0 };
    for (name, value, http_only) in [
        (
            session_name,
            credential.unwrap_or("").to_owned(),
            "; HttpOnly",
        ),
        (csrf_name, credential.map(csrf).unwrap_or_default(), ""),
    ] {
        response.headers_mut().append(
            "set-cookie",
            format!("{name}={value}; Path=/; Max-Age={age}; SameSite=Lax{flags}{http_only}")
                .parse()
                .unwrap(),
        );
    }
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
}
pub async fn apply(
    State(a): State<veyndo_core::State>,
    mut request: Request,
    next: Next,
) -> Response {
    let browser = request
        .headers()
        .get("x-veyndo-browser-session")
        .is_some_and(|v| v == "1");
    if !request.uri().path().starts_with("/api/")
        && !(browser
            && (request.uri().path().starts_with("/store-api/") || request.uri().path() == "/mcp"))
    {
        return next.run(request).await;
    }
    let Some((expected, secure)) = origin(&request) else {
        if browser {
            return Error(
                StatusCode::FORBIDDEN,
                "Browser session origin is not admitted".into(),
            )
            .into_response();
        }
        return next.run(request).await;
    };
    let path = request.uri().path().to_owned();
    let auth = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned);
    let credential = cookie(request.headers(), names(secure).0);
    let entry = matches!(
        path.as_str(),
        "/api/auth/login"
            | "/api/auth/register"
            | "/api/auth/accept"
            | "/api/auth/redeem"
            | "/api/auth/mfa/verify"
            | "/api/identity/session"
    );
    let using_cookie =
        browser && !entry && credential.is_some() && auth.as_deref().is_none_or(|s| s == MARKER);
    if browser || using_cookie {
        if !same_origin(&request, &expected) {
            return Error(
                StatusCode::FORBIDDEN,
                "Browser session requires the exact Studio origin".into(),
            )
            .into_response();
        }
        if using_cookie
            && !request
                .headers()
                .get("x-veyndo-csrf")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|s| equal(s, &csrf(credential.as_ref().unwrap())))
        {
            if path == "/api/auth/logout" && request.method() == axum::http::Method::POST {
                // Leaving a stale tab clears this browser, without revoking another account's ledger.
                let mut response = Json(json!({"loggedOut":true})).into_response();
                set_cookies(&mut response, None, secure);
                return response;
            }
            return Error(
                StatusCode::UNAUTHORIZED,
                "Browser session changed or CSRF proof is invalid".into(),
            )
            .into_response();
        }
    }
    if using_cookie {
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {}", credential.as_ref().unwrap())
                .parse()
                .unwrap(),
        );
    } else if auth.as_deref() == Some(MARKER) {
        request.headers_mut().remove("authorization");
    }
    let mut response = next.run(request).await;
    if (browser || using_cookie) && path == "/api/auth/logout" {
        if response.status() == StatusCode::UNAUTHORIZED {
            response = Json(json!({"loggedOut":true})).into_response();
        }
        set_cookies(&mut response, None, secure);
        return response;
    }
    if !browser
        || !response.status().is_success()
        || !(path.starts_with("/api/auth/") || path == "/api/identity/session")
    {
        if using_cookie {
            response
                .headers_mut()
                .insert("cache-control", "no-store".parse().unwrap());
        }
        return response;
    }
    bind_session(
        &a,
        response,
        &path,
        (credential, auth),
        (using_cookie, secure),
    )
    .await
}
/// Replaces the session token of a successful sign-in answer by the cookie marker and sets the host-only cookies.
async fn bind_session(
    a: &veyndo_core::State,
    response: Response,
    path: &str,
    (credential, auth): (Option<String>, Option<String>),
    (using_cookie, secure): (bool, bool),
) -> Response {
    let (parts, body) = response.into_parts();
    let Ok(bytes) = to_bytes(body, 1024 * 1024).await else {
        return bad("Browser authentication response exceeds its limit").into_response();
    };
    let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let session = if value.get("session").is_some_and(Value::is_object) {
        &mut value["session"]
    } else {
        &mut value
    };
    let issued = session["token"].as_str().is_some_and(token);
    // Passive bearer/session reads must not overwrite a later explicit browser login.
    if !issued && !using_cookie {
        return Response::from_parts(parts, Body::from(bytes));
    }
    let candidate = session["token"].as_str().map(str::to_owned).or_else(|| {
        (path == "/api/auth/session")
            .then(|| {
                if using_cookie {
                    credential.clone()
                } else {
                    auth.clone()
                }
            })
            .flatten()
    });
    let Some(candidate) = candidate.filter(|s| token(s)) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    if session["user"]["id"].as_str().is_none() {
        return bad("Invalid browser authentication response").into_response();
    }
    let user = session["user"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let valid = veyndo_runtime::tenant_scope::scoped(
        veyndo_runtime::tenant_scope::Scope::System,
        crate::store::sessions::live(a, &hash(&candidate), &user),
    )
    .await;
    if !matches!(valid, Ok(true)) {
        return Error(
            StatusCode::UNAUTHORIZED,
            "Personal browser session unavailable".into(),
        )
        .into_response();
    }
    session["token"] = json!(MARKER);
    session["browserSession"] = json!(true);
    session["browserCsrf"] = json!(csrf(&candidate));
    let mut response = Response::from_parts(parts, Body::from(value.to_string()));
    response.headers_mut().remove("content-length");
    if issued || !using_cookie {
        set_cookies(&mut response, Some(&candidate), secure);
    } else {
        response
            .headers_mut()
            .insert("cache-control", "no-store".parse().unwrap());
    }
    response
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csrf_is_domain_separated_and_duplicate_cookies_are_not_authority() {
        let session = "a".repeat(64);
        assert_ne!(csrf(&session), session);
        assert!(equal(&csrf(&session), &csrf(&session)));
        assert!(!equal("wrong", &csrf(&session)));
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            "cookie",
            format!("veyndo-session={session}; veyndo-session={session}")
                .parse()
                .unwrap(),
        );
        assert!(cookie(&h, "veyndo-session").is_none());
        h.insert(
            "cookie",
            format!("veyndo-session={session}").parse().unwrap(),
        );
        assert_eq!(cookie(&h, "veyndo-session"), Some(session));
    }
}
