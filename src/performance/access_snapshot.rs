//! One fresh, server-owned admission read shared by domain, identity, availability, channel and proxy middleware.
use crate::*;

pub(crate) use veyndo_core::access::{AccessSnapshot, ChannelSnapshot, FrontendMount};

pub(crate) async fn load(
    a: &App,
    alias: Option<&str>,
    tenant: &str,
    channel: &str,
) -> Result<Arc<AccessSnapshot>> {
    let r = sqlx::query(include_str!("access_snapshot.sql"))
        .bind(alias)
        .bind(tenant)
        .bind(channel)
        .fetch_one(&a.db)
        .await?;
    Ok(Arc::new(AccessSnapshot {
        tenant: r.get("tenant"),
        channel_id: r.get("channel"),
        exists: r.get("exists"),
        parent: r.get("parent"),
        status: r.get("status"),
        channel: r
            .get::<Option<Value>, _>("channel_data")
            .map(|data| ChannelSnapshot {
                data,
                revision: r.get("channel_revision"),
            }),
        mount: r
            .get::<Option<String>, _>("origin")
            .map(|origin| FrontendMount {
                origin,
                experience_alias: r.get("experience_alias"),
            }),
    }))
}
/// Whether the snapshot was read for the shop this request addresses.
pub(crate) fn matches(snapshot: &AccessSnapshot, h: &RequestContext) -> bool {
    tenant(h).is_ok_and(|t| t == snapshot.tenant)
}
