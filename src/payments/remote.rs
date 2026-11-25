//! Dedicated payment RPC, pinned service version and operator-owned egress; never ordinary app-action receipts.
use super::Attempt;
use crate::foundation::App;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::http_limits;
/// The configured payment service address and its bearer credential.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Endpoint {
    #[serde(deserialize_with = "lenient")]
    url: Option<String>,
    #[serde(deserialize_with = "lenient")]
    token: Option<String>,
}

pub(crate) use veyndo_payments::store::provider_registry::config;
pub(crate) async fn call(
    a: &App,
    t: &str,
    provider: &str,
    version: &str,
    environment: &str,
    path: &str,
    body: &Value,
) -> Result<Value> {
    if veyndo_core::store::environments::parent(a, t)
        .await?
        .is_some()
    {
        return Err(bad("External payments are disabled in private sandboxes"));
    }
    let c: Endpoint = view(&config(provider, version, environment)?);
    let _cluster =
        crate::performance::cluster_lease::Lease::acquire(a, t, "payment-provider", 8).await?;
    let _admission = crate::apps::service_limits::admit(a, t, provider).await?;
    if body.to_string().len() > 65536 {
        return Err(bad("Payment request exceeds limit"));
    }
    // The shared client disables redirects and reuses its connection pool.
    let r = a
        .http
        .post(format!(
            "{}/v1/{path}",
            c.url.as_deref().unwrap().trim_end_matches('/')
        ))
        .timeout(std::time::Duration::from_secs(15))
        .bearer_auth(c.token.as_deref().unwrap())
        .header("x-tenant", t)
        .json(body)
        .send()
        .await
        .map_err(|_| {
            Error(
                StatusCode::BAD_GATEWAY,
                "Payment service outcome uncertain; reconcile with the same request key".into(),
            )
        })?;
    if !r.status().is_success() {
        let diagnostic = http_limits::json_body(r)
            .await
            .map(|body| super::diagnostics::response(&body))
            .unwrap_or("Payment service rejected operation");
        return Err(Error(StatusCode::BAD_GATEWAY, diagnostic.into()));
    }
    http_limits::json_body(r).await
}
pub(crate) async fn execute(
    a: &App,
    p: &Attempt,
    op: &str,
    key: &str,
    input: &Value,
) -> Result<Value> {
    veyndo_payments::service::remote::execute(&super::host::Gateway(a), a, p, op, key, input).await
}
