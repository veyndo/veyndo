//! Application dependencies, error responses and request context helpers.
use crate::*;

#[derive(Clone)]
pub(crate) struct App {
    pub(crate) db: PgPool,
    pub(crate) _connection_budget: Arc<performance::cluster_lease::Lease>,
    pub(crate) inference_slots: Arc<tokio::sync::Semaphore>,
    pub(crate) token: Arc<String>,
    pub(crate) http: reqwest::Client,
    pub(crate) inference: Inference,
    pub(crate) model: Arc<String>,
    pub(crate) sandboxes: Arc<sandbox_cache::Cache>,
    pub(crate) channel_metrics: Arc<channel_metrics::ChannelMetrics>,
    pub(crate) app_limits: Arc<apps::ServiceLimits>,
    pub(crate) admission: Arc<performance::Admission>,
    pub(crate) reads: Arc<performance::Reads>,
}
pub(crate) use veyndo_core::{Error, Result, bad, conflict, hash, uid, validate_tenant};
pub(crate) fn header<'a>(h: &'a impl request_context::HeaderReader, k: &str) -> Option<&'a str> {
    h.value(k)
}
pub(crate) fn tenant(h: &impl request_context::HeaderReader) -> Result<String> {
    let t = header(h, "x-tenant")
        .or(runtime_config::get().default_tenant.as_deref())
        .ok_or(bad(
            "Shop scope required; use its domain or x-tenant header",
        ))?;
    validate_tenant(t)?;
    Ok(t.into())
}
pub(crate) fn merchant(_a: &App, h: &RequestContext) -> Result<String> {
    auth::permit(h, "read")?;
    let t = header(h, "x-rac-tenant").ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Merchant credential required".into(),
    ))?;
    if t != tenant(h)? {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Workspace does not match authenticated context".into(),
        ));
    }
    Ok(t.into())
}
pub(crate) fn token(h: &RequestContext) -> Result<&str> {
    header(h, "sw-context-token").ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Customer context required".into(),
    ))
}
