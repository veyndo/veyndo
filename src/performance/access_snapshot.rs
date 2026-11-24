//! The per-request admission snapshot shared by domain, identity, availability, channel and proxy middleware; the shop component reads it.
use crate::*;

pub(crate) use veyndo_core::access::AccessSnapshot;

pub(crate) async fn load(
    a: &App,
    alias: Option<&str>,
    tenant: &str,
    channel: &str,
) -> Result<Arc<AccessSnapshot>> {
    veyndo_shop::store::access_snapshot::load(a, alias, tenant, channel).await
}
/// Whether the snapshot was read for the shop this request addresses.
pub(crate) fn matches(snapshot: &AccessSnapshot, h: &RequestContext) -> bool {
    tenant(h).is_ok_and(|t| t == snapshot.tenant)
}
