//! Dedicated payment RPC, pinned service version and operator-owned egress; never ordinary app-action receipts.
use super::Attempt;
use crate::{App, Error, Result, StatusCode, Value, bad, http_limits};

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
    if veyndo_core::shop_scope::parent(a, t).await?.is_some() {
        return Err(bad("External payments are disabled in private sandboxes"));
    }
    let c = config(provider, version, environment)?;
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
            c["url"].as_str().unwrap().trim_end_matches('/')
        ))
        .timeout(std::time::Duration::from_secs(15))
        .bearer_auth(c["token"].as_str().unwrap())
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
