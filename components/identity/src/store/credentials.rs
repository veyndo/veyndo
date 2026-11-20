//! Resolves a bearer credential once: the person or app behind it, its workspace memberships and the selected shop.
use serde_json::Value;
use sqlx::Row;
use veyndo_core::access::ChannelSnapshot;
use veyndo_core::{Error, Result, State, StatusCode, bad, hash};

#[derive(serde::Deserialize)]
pub struct Member {
    pub tenant: String,
    pub role: String,
    pub permissions: Value,
}
pub struct Identity {
    pub user: String,
    pub default: String,
    pub scopes: Option<Vec<String>>,
    pub app: Option<String>,
    pub app_permissions: Option<Value>,
    pub agent_key: Option<String>,
    pub key_expiry: Option<i64>,
    pub tenant: Option<String>,
    pub parent: Option<String>,
    pub preview_owner: Option<String>,
    pub preview_valid: Option<bool>,
    pub status: Option<String>,
    pub members: Vec<Member>,
    pub channel: Option<ChannelSnapshot>,
}
pub async fn resolve_identity(a: &State, token: &str, selected: Option<&str>) -> Result<Identity> {
    resolve_identity_channel(a, token, selected, None).await
}
pub async fn resolve_identity_channel(
    a: &State,
    token: &str,
    selected: Option<&str>,
    channel: Option<&str>,
) -> Result<Identity> {
    let r = veyndo_runtime::tenant_scope::scoped(
        veyndo_runtime::tenant_scope::Scope::System,
        sqlx::query(include_str!("identity.sql"))
            .bind(hash(token))
            .bind(selected)
            .bind(channel)
            .fetch_optional(&a.db),
    )
    .await?
    .ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Session or integration key expired or invalid".into(),
    ))?;
    let scopes: Option<Value> = r.get("scopes");
    Ok(Identity {
        user: r.get("user_id"),
        app: r.get("app_id"),
        app_permissions: r.get("app_permissions"),
        agent_key: r.get("agent_key"),
        key_expiry: r.get("key_expiry"),
        default: r.get("default_tenant"),
        tenant: r.get("tenant"),
        parent: r.get("parent"),
        preview_owner: r.get("preview_owner"),
        preview_valid: r.get("preview_valid"),
        status: r.get("status"),
        channel: r
            .get::<Option<Value>, _>("channel_data")
            .map(|data| ChannelSnapshot {
                data,
                revision: r.get("channel_revision"),
            }),
        scopes: scopes
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| bad("Invalid integration scopes"))?,
        members: serde_json::from_value(r.get("memberships"))
            .map_err(|_| bad("Invalid membership grants"))?,
    })
}
pub async fn resolve_credential(
    a: &State,
    token: &str,
) -> Result<(String, String, Option<Vec<String>>)> {
    let identity = resolve_identity(a, token, None).await?;
    Ok((identity.user, identity.default, identity.scopes))
}
