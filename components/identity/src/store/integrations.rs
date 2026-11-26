//! Workspace integration (API/MCP) keys: the list, the active-key count, creation with a bounded or infinite lifetime and revocation. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// An integration key as the key list shows it; the secret itself is never stored.
pub struct KeyRow {
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub app_id: Option<String>,
    pub permissions: Value,
    pub agent_budget: Option<Value>,
    pub agent_tokens_allocated: i64,
    pub agent_cost_allocated: i64,
    pub created: String,
    pub expires: Option<String>,
}

/// The shop's keys, newest first (at most 100); `expires` is absent for a non-expiring key.
pub async fn list(a: &State, tenant: &str) -> Result<Vec<KeyRow>> {
    Ok(sqlx::query_as!(
        KeyRow,
        r#"SELECT id,name,user_id,app_id,permissions,agent_budget,agent_tokens_allocated,agent_cost_allocated,created_at::text AS "created!",CASE WHEN isfinite(expires_at) THEN expires_at::text ELSE NULL END AS expires FROM integration_keys WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100"#,
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}

/// How many unexpired keys the shop holds.
pub async fn active_count(tx: &mut sqlx::PgConnection, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM integration_keys WHERE tenant=$1 AND expires_at>now()"#,
        tenant
    )
    .fetch_one(tx)
    .await?)
}

/// A key to create: who, what it may do, how long it lives and its optional agent budget.
pub struct NewKey<'a> {
    pub id: &'a str,
    pub tenant: &'a str,
    pub user: &'a str,
    pub digest: &'a str,
    pub name: &'a str,
    pub permissions: &'a Value,
    pub days: i32,
    pub non_expiring: bool,
    pub agent_budget: Option<Value>,
}

/// Stores a key under the digest of its secret; a non-expiring key gets an infinite expiry.
pub async fn create(tx: &mut sqlx::PgConnection, key: NewKey<'_>) -> Result<()> {
    sqlx::query!(
        "INSERT INTO integration_keys(id,tenant,user_id,digest,name,permissions,expires_at,agent_budget) VALUES($1,$2,$3,$4,$5,$6,CASE WHEN $8 THEN 'infinity'::timestamptz ELSE now()+$7*interval '1 day' END,$9)",
        key.id,
        key.tenant,
        key.user,
        key.digest,
        key.name,
        key.permissions,
        key.days as _,
        key.non_expiring,
        key.agent_budget
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Revokes a key: a budgeted agent key expires now (its usage ledger stays), any other key is deleted.
pub async fn revoke(tx: &mut sqlx::PgConnection, tenant: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE integration_keys SET expires_at=least(expires_at,now()) WHERE tenant=$1 AND id=$2 AND agent_budget IS NOT NULL",
        tenant,
        id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM integration_keys WHERE tenant=$1 AND id=$2 AND agent_budget IS NULL",
        tenant,
        id
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}
