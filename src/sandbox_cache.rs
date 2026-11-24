//! Tenant business policy preparation; the apps component owns the policy and its bounded compilation cache.
use crate::{App, Result, Sandbox};
use std::sync::Arc;
pub(crate) use veyndo_app_platform::service::sandbox_cache::Cache;
pub(crate) async fn prepare(a: &App, tenant: &str) -> Result<Arc<Sandbox>> {
    veyndo_app_platform::service::business_policy::prepare(a, &a.sandboxes, tenant).await
}
