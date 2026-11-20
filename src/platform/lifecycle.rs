//! Audited, reversible lifecycle; preserved commerce records and current revision prevent accidental overwrites.
use super::*;
pub(super) use veyndo_shop::http::operator_lifecycle::change;
/// Evaluate availability after identity derivation. Paused shops retain merchant GET access; deleted shops require restoration.
pub(crate) async fn admit(a: &App, h: &RequestContext, path: &str, method: &str) -> Result<bool> {
    if path.starts_with("/api/auth/")
        || path.starts_with("/api/platform/")
        || path.starts_with("/api/identity/")
        || path == "/api/capabilities"
        || path == "/api/workspaces"
        || path == "/health"
        || path.starts_with("/assets/")
        || !["/api/", "/store-api/", "/ucp/", "/mcp", "/webhooks/"]
            .iter()
            .any(|prefix| path.starts_with(prefix))
    {
        return Ok(false);
    }
    let t = tenant(h)?;
    let status = if h.principal.tenant.as_deref() == Some(t.as_str()) && h.tenant_status.is_some() {
        h.tenant_status.clone()
    } else if let Some(access) = h
        .access
        .as_ref()
        .filter(|s| crate::performance::access_snapshot::matches(s, h))
    {
        access.status.clone()
    } else {
        sqlx::query_scalar::<_,String>("SELECT t.status FROM tenants t WHERE t.id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1)").bind(&t).fetch_optional(&a.db).await?
    };
    let Some(status) = status else {
        return Err(Error(StatusCode::NOT_FOUND, "Shop not found".into()));
    };
    let read = (method == "GET"
        || method == "POST"
            && [
                "/api/search/product",
                "/api/search/order",
                "/api/merchant/quote",
            ]
            .contains(&path))
        && header(h, "x-rac-user").is_some()
        && path.starts_with("/api/");
    // Reconciliation callbacks for already-created payments must remain usable during suspension/trash.
    // Route exemption is availability only: the handler still verifies HMAC and original attempt identity.
    let settlement = method == "POST"
        && (path == "/store-api/payments/paypal/webhooks"
            || path
                .strip_prefix("/store-api/payment-providers/")
                .is_some_and(|tail| {
                    tail.strip_suffix("/webhooks")
                        .is_some_and(|id| !id.is_empty() && !id.contains('/'))
                }));
    if !verified_kernel::shop_request_admissible(
        status == "active",
        status == "paused",
        read,
        settlement,
    ) {
        return Err(Error(
            if status == "archived" {
                StatusCode::GONE
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            },
            "Shop is unavailable; contact the platform operator".into(),
        ));
    }
    Ok(true)
}
