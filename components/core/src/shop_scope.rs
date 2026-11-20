//! Which shop a request addresses: the transport header, or the deployment's default shop. No authentication.
use crate::{
    Error, RequestContext, Result, StatusCode, bad, request_context::HeaderReader, validate_tenant,
};
use std::{env, sync::OnceLock};

/// A deployment flag that is exactly `true` or `false`; anything else stops the process at startup.
pub fn env_flag(key: &str, default: bool) -> bool {
    match env::var(key).as_deref() {
        Ok("true") => true,
        Ok("false") => false,
        Err(_) => default,
        _ => panic!("Invalid boolean {key}"),
    }
}
/// The shop used when a request names none: configured explicitly, or the demo shop outside strict deployments.
pub fn default_tenant() -> Option<&'static str> {
    static DEFAULT: OnceLock<Option<String>> = OnceLock::new();
    DEFAULT
        .get_or_init(|| {
            let strict = env_flag("DB_RLS_REQUIRED", false);
            let tenant = env::var("PUBLIC_DEFAULT_TENANT")
                .ok()
                .or_else(|| (!strict && env_flag("SEED_DEMO", true)).then(|| "atelier".into()));
            if let Some(t) = &tenant {
                assert!(validate_tenant(t).is_ok(), "Invalid PUBLIC_DEFAULT_TENANT");
            }
            tenant
        })
        .as_deref()
}
pub fn header<'a>(h: &'a impl HeaderReader, k: &str) -> Option<&'a str> {
    h.value(k)
}
pub fn tenant(h: &impl HeaderReader) -> Result<String> {
    let t = header(h, "x-tenant").or(default_tenant()).ok_or(bad(
        "Shop scope required; use its domain or x-tenant header",
    ))?;
    validate_tenant(t)?;
    Ok(t.into())
}
/// The sales channel a storefront request addresses.
pub fn channel(h: &RequestContext) -> &str {
    header(h, "sw-sales-channel-id").unwrap_or("default")
}
pub fn token(h: &RequestContext) -> Result<&str> {
    header(h, "sw-context-token").ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Customer context required".into(),
    ))
}
/// The stored digest of the customer session a request presents; a checkout buyer carries its own.
pub fn session_digest(h: &RequestContext) -> Result<String> {
    if let Some(buyer) = &h.checkout_buyer {
        return Ok(buyer.digest.clone());
    }
    let token = header(h, "x-customer-token").ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Customer login required".into(),
    ))?;
    Ok(crate::hash(token))
}
/// The live shop a staging environment belongs to; `None` for a live shop.
pub async fn live_parent(conn: &mut sqlx::PgConnection, t: &str) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT live_tenant FROM shop_environments WHERE tenant=$1")
            .bind(t)
            .fetch_optional(conn)
            .await?,
    )
}
/// The live shop a staging environment belongs to, read on the pooled connection; `None` for a live shop.
pub async fn parent(a: &crate::State, t: &str) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT live_tenant FROM shop_environments WHERE tenant=$1")
            .bind(t)
            .fetch_optional(&a.db)
            .await?,
    )
}
