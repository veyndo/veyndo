//! One fresh admission read per request: the hosted mount, the addressed shop, its live environment status and the sales channel.
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;
use veyndo_core::access::{AccessSnapshot, ChannelSnapshot, FrontendMount};
use veyndo_core::{Result, State};

/// Reads the snapshot under the request's MVCC view; a hosted alias decides the shop and channel, never headers.
pub async fn load(
    a: &State,
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
