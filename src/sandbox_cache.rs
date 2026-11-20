//! Tenant business policy preparation on the bounded compilation cache of the apps component.
use crate::{App, Result, Sandbox, bad, conflict, hash};
use sqlx::Row;
use std::sync::Arc;
pub(crate) use veyndo_app_platform::service::sandbox_cache::Cache;
pub(crate) async fn prepare(a: &App, tenant: &str) -> Result<Arc<Sandbox>> {
    let row = sqlx::query("SELECT digest,wat FROM extensions WHERE tenant=$1")
        .bind(tenant)
        .fetch_optional(&a.db)
        .await?
        .ok_or(bad("Business approval policy missing"))?;
    let digest: String = row.get("digest");
    if let Some(cached) = a.sandboxes.get(tenant, &digest) {
        return Ok(cached);
    }
    let source: String = row.get("wat");
    if hash(&source) != digest {
        return Err(conflict(
            "Business policy digest mismatch; ask the merchant to reactivate its policy",
        ));
    }
    a.sandboxes.compile(tenant, source).await
}
