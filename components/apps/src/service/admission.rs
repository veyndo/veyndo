//! Shared network-app admission: the local bulkheads plus replica-wide fenced leases.
use crate::ports::Platform;
use tokio::sync::OwnedSemaphorePermit;
use veyndo_core::Result;
use veyndo_core::database::lease::Lease;

/// Held for the duration of one app network call.
pub struct Admission {
    _local: (
        OwnedSemaphorePermit,
        OwnedSemaphorePermit,
        OwnedSemaphorePermit,
    ),
    _tenant: Lease,
    _app: Lease,
}
/// All app-network entry points share these ceilings, including signed delivery and pinned UI fetches.
pub async fn admit(a: &Platform, tenant: &str, app: &str) -> Result<Admission> {
    let local = a.limits.enter(tenant, app)?;
    let tenant_lease = Lease::acquire(a, tenant, "app-network", 16).await?;
    let app_lease = Lease::acquire(a, tenant, &format!("app:{app}"), 8).await?;
    Ok(Admission {
        _local: local,
        _tenant: tenant_lease,
        _app: app_lease,
    })
}
