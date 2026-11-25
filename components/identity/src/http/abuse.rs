//! Database-backed account and trusted peer-prefix throttles shared by replicas; reserve before password hashing.
use crate::store::attempts;
use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::{IpAddr, SocketAddr};
use veyndo_core::admission::run as admit_request;
use veyndo_core::request_context::HeaderReader;
use veyndo_core::{Error, Result, State, StatusCode, bad, hash, process_config};
use veyndo_runtime::tenant_scope::{Scope, scoped};
/// The network prefix of an address: /24 for IPv4, /56 for IPv6, never the address itself.
fn prefix(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(ip) => {
            let b = ip.octets();
            format!("{}.{}.{}", b[0], b[1], b[2])
        }
        IpAddr::V6(ip) => {
            let b = ip.octets();
            b[..7].iter().map(|b| format!("{b:02x}")).collect()
        }
    }
}
/// The peer prefix of a request; a forwarded address counts only behind a trusted proxy.
fn peer(request: &Request) -> String {
    let actual = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|p| p.0.ip());
    let trusted = actual.is_some_and(|ip| process_config::get().trusted_proxies.contains(&ip));
    // Only the last proxy-added address is trusted; user-supplied leading forwarded values are ignored.
    let forwarded = if trusted {
        request
            .headers()
            .value("x-forwarded-for")
            .and_then(|s| s.rsplit(',').next())
            .and_then(|s| s.trim().parse().ok())
    } else {
        None
    };
    forwarded
        .or(actual)
        .map(prefix)
        .unwrap_or_else(|| "unknown-peer".into())
}
/// Counts one attempt in the bucket; a full or blocked bucket refuses it.
async fn reserve(a: &State, key: &str, limit: i32) -> Result<()> {
    let allowed = attempts::reserve(a, key, limit).await?;
    if allowed.is_none() {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many sign-in attempts. Wait a minute before trying again.".into(),
        ));
    }
    Ok(())
}
/// Throttles sign-in, registration and redemption by peer prefix and account before the request is admitted.
pub async fn run(a: &State, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let guarded = matches!(
        path.as_str(),
        "/api/auth/login"
            | "/api/auth/mfa/verify"
            | "/api/auth/mfa/enroll"
            | "/api/auth/mfa/confirm"
            | "/api/auth/mfa/disable"
            | "/api/auth/register"
            | "/api/auth/accept"
            | "/api/auth/redeem"
            | "/store-api/account/login"
            | "/store-api/account/register"
    );
    if !guarded {
        return admit_request(a, request, next).await;
    }
    let peer = hash(&format!("auth-ip:{}", peer(&request)));
    let checked = scoped(Scope::System, reserve(a, &peer, 120)).await;
    if let Err(e) = checked {
        return throttled(e);
    }
    let (parts, body) = request.into_parts();
    let body = match axum::body::to_bytes(body, 65536).await {
        Ok(b) => b,
        Err(_) => return bad("Authentication payload exceeds 64 KiB").into_response(),
    };
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    let account = value["email"]
        .as_str()
        .map(|s| s.trim().to_lowercase())
        .or_else(|| {
            value["invitationToken"]
                .as_str()
                .or(value["token"].as_str())
                .map(|s| format!("token:{}", hash(s)))
        });
    let tenant = if path.starts_with("/store-api/") {
        parts.headers.value("x-tenant").unwrap_or("unknown-shop")
    } else {
        "global"
    };
    let key = account.map(|account| hash(&format!("auth-account:{path}:{tenant}:{account}")));
    if let Some(key) = &key
        && let Err(e) = scoped(Scope::System, reserve(a, key, 12)).await
    {
        return throttled(e);
    }
    let response = admit_request(a, Request::from_parts(parts, Body::from(body)), next).await;
    if response.status() == StatusCode::UNAUTHORIZED
        && let Some(key) = key
    {
        let _ = scoped(Scope::System, attempts::block(a, &key)).await;
    }
    response
}
/// A refused attempt, with a retry hint when it was throttled.
fn throttled(e: Error) -> Response {
    let mut r = e.into_response();
    if r.status() == StatusCode::TOO_MANY_REQUESTS {
        r.headers_mut().insert("retry-after", "60".parse().unwrap());
    }
    r
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_group_addresses_without_storing_personal_addresses() {
        assert_eq!(
            prefix("192.0.2.4".parse().unwrap()),
            prefix("192.0.2.200".parse().unwrap())
        );
        assert_ne!(
            prefix("192.0.3.4".parse().unwrap()),
            prefix("192.0.2.4".parse().unwrap())
        );
        assert_eq!(
            prefix("2001:db8:abcd:1200::1".parse().unwrap()),
            prefix("2001:db8:abcd:12ff::8".parse().unwrap())
        );
    }
}
