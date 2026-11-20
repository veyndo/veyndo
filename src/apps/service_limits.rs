//! Shared network-app admission: the local bulkheads of the apps component plus replica-wide fenced leases.
use super::*;
use tokio::sync::OwnedSemaphorePermit;
pub(crate) use veyndo_app_platform::service::limits::ServiceLimits;
pub(crate) struct Admission {
    _local: (
        OwnedSemaphorePermit,
        OwnedSemaphorePermit,
        OwnedSemaphorePermit,
    ),
    _tenant: crate::performance::cluster_lease::Lease,
    _app: crate::performance::cluster_lease::Lease,
}
/// All app-network entry points share these ceilings, including signed delivery and pinned UI fetches.
pub(crate) async fn admit(a: &App, tenant: &str, app: &str) -> Result<Admission> {
    let local = a.app_limits.enter(tenant, app)?;
    let tenant_lease =
        crate::performance::cluster_lease::Lease::acquire(a, tenant, "app-network", 16).await?;
    let app_lease =
        crate::performance::cluster_lease::Lease::acquire(a, tenant, &format!("app:{app}"), 8)
            .await?;
    Ok(Admission {
        _local: local,
        _tenant: tenant_lease,
        _app: app_lease,
    })
}
