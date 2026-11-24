//! Tenant business policy preparation; the apps component owns the policy and its bounded compilation cache.
use crate::foundation::App;
use std::sync::Arc;
use veyndo_apps::sandbox::Sandbox;
use veyndo_core::Result;
pub(crate) async fn prepare(a: &App, tenant: &str) -> Result<Arc<Sandbox>> {
    veyndo_app_platform::service::business_policy::prepare(a, &a.apps.sandboxes, tenant).await
}
