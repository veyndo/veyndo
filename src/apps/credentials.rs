//! App keys reuse the core integration-key store, current creator rights, expiry and immutable package digest.
use super::*;
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/credentials",
            &[("GET", "apps.manage"), ("POST", "apps.manage")],
            get(veyndo_app_platform::http::credentials::list)
                .post(veyndo_app_platform::http::credentials::create),
        )
        .secure_route(
            "/api/apps/{id}/credentials/{key}",
            &[("DELETE", "apps.manage")],
            axum::routing::delete(veyndo_app_platform::http::credentials::revoke),
        )
}
pub(super) use veyndo_app_model::admission::callback_scope as scope;

pub(crate) async fn admit_catalog_sync(a: &App, h: &RequestContext, tenant: &str) -> Result<bool> {
    let id = "knowledge_catalog";
    if h.principal.app.as_deref() != Some(id) || permit(h, "products.read").is_err() {
        return Ok(false);
    }
    let m = package(a, tenant, id, true).await?;
    Ok(approval::approved(
        &m,
        &crate::runtime_config::get().services[id],
    ))
}

pub(super) use veyndo_app_platform::domain::callback_grant::permit;
