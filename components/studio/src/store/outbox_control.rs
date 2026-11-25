//! Outbox runtime SQL: tenant counters and the explicit release of one quarantined event that was neither delivered nor retired.
use veyndo_core::{Result, State};

/// Pending, quarantined, retrying and consumed event counts of one shop.
pub async fn counters(a: &State, tenant: &str) -> Result<[i64; 4]> {
    veyndo_core::store::outbox_control::counters(a, tenant).await
}

/// Releases a quarantined event and its quarantined subscriber lanes; false when nothing in this shop was quarantined.
pub async fn release(a: &State, tenant: &str, id: i64) -> Result<bool> {
    veyndo_core::store::outbox_control::release(a, tenant, id).await
}
