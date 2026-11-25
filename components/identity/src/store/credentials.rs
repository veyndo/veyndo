//! Resolves a bearer credential once: the person or app behind it, its workspace memberships and the selected shop.
use serde_json::Value;
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
/// The credential snapshot `identity.sql` returns, checked against the schema at compile time.
struct IdentityRow {
    user_id: String,
    default_tenant: String,
    scopes: Option<Value>,
    app_id: Option<String>,
    app_permissions: Option<Value>,
    agent_key: Option<String>,
    key_expiry: Option<i64>,
    tenant: Option<String>,
    parent: Option<String>,
    preview_owner: Option<String>,
    preview_valid: Option<bool>,
    status: Option<String>,
    channel_data: Option<Value>,
    channel_revision: Option<i64>,
    memberships: Value,
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
        sqlx::query_file_as!(
            IdentityRow,
            "src/store/identity.sql",
            hash(token),
            selected,
            channel
        )
        .fetch_optional(&a.db),
    )
    .await?
    .ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Session or integration key expired or invalid".into(),
    ))?;
    let scopes = r.scopes;
    Ok(Identity {
        user: r.user_id,
        app: r.app_id,
        app_permissions: r.app_permissions,
        agent_key: r.agent_key,
        key_expiry: r.key_expiry,
        default: r.default_tenant,
        tenant: r.tenant,
        parent: r.parent,
        preview_owner: r.preview_owner,
        preview_valid: r.preview_valid,
        status: r.status,
        channel: r.channel_data.map(|data| ChannelSnapshot {
            data,
            revision: r.channel_revision.unwrap_or_default(),
        }),
        scopes: scopes
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| bad("Invalid integration scopes"))?,
        members: serde_json::from_value(r.memberships)
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
