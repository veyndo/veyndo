//! Calls through a typed app port: the consumer names the interface, the platform resolves the installed provider and runs its action through the same permission-aware gateway, so the caller's rights, tenant, quotas and egress rules still apply.
use crate::domain::input_schema::validate_native;
use crate::domain::port_wiring::{Installed, resolve};
use crate::ports::Platform;
use crate::service::gateway;
use crate::store::{package_contracts, packages::package};
use serde::Serialize;
use serde_json::Value;
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, StatusCode, conflict};

/// The result of a port call with the provider that served it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortResult {
    pub provider: String,
    pub version: String,
    pub result: Value,
}
/// Calls `port` on behalf of the active app `consumer`.
pub async fn call(
    a: &Platform,
    h: &RequestContext,
    consumer: &str,
    port: &str,
    input: &Value,
) -> Result<PortResult> {
    let t = tenant(h)?;
    package(a, &t, consumer, true).await?;
    let contracts = package_contracts::active(&a.db, &t).await?;
    let wanted = contracts
        .iter()
        .find(|(app, _)| app == consumer)
        .and_then(|(_, c)| c.consumes.iter().find(|p| p.name == port))
        .ok_or(Error(
            StatusCode::FORBIDDEN,
            "The app does not declare this port".into(),
        ))?;
    let installed: Vec<Installed<'_>> = contracts
        .iter()
        .map(|(app, contract)| Installed { app, contract })
        .collect();
    let (provider, provided) = resolve(consumer, wanted, &installed).ok_or(conflict(
        "No installed app provides this port in a compatible version",
    ))?;
    validate_native(&provided.input_schema, input)?;
    let result = gateway::invoke(a, h, provider, &provided.action, input, None).await?;
    Ok(PortResult {
        provider: provider.into(),
        version: provided.version.clone(),
        result,
    })
}
