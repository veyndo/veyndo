//! App callback keys in the integration-key table: list, count, issue and revoke; never a key value.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The callback keys of an app as metadata objects, newest first (at most 100).
pub async fn list(a: &State, t: &str, app: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT jsonb_build_object('id',id,'permissions',app_permissions,'createdAt',created_at,'expiresAt',expires_at,'creator',user_id,'packageDigest',package_digest) AS "key!" FROM integration_keys WHERE tenant=$1 AND app_id=$2 ORDER BY created_at DESC LIMIT 100"#,
        t,
        app
    )
    .fetch_all(&a.db)
    .await?)
}

/// How many integration keys of the shop have not expired.
pub async fn active_count(conn: &mut PgConnection, t: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM integration_keys WHERE tenant=$1 AND expires_at>now()"#,
        t
    )
    .fetch_one(conn)
    .await?)
}

/// A callback key to issue: its hashed value, core scopes and the approved app package.
pub struct NewKey<'a> {
    pub id: &'a str,
    pub tenant: &'a str,
    pub user: &'a str,
    pub digest: &'a str,
    pub name: &'a str,
    pub scopes: &'a Value,
    pub app: &'a str,
    pub package_digest: &'a str,
    pub days: i32,
    pub app_permissions: &'a Value,
}

/// Stores a callback key that expires after `days`.
pub async fn issue(conn: &mut PgConnection, k: NewKey<'_>) -> Result<()> {
    sqlx::query!(
        "INSERT INTO integration_keys(id,tenant,user_id,digest,name,permissions,app_id,package_digest,app_permissions,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$10,now()+$9::int4*interval '1 day')",
        k.id,
        k.tenant,
        k.user,
        k.digest,
        k.name,
        k.scopes,
        k.app,
        k.package_digest,
        k.days,
        k.app_permissions
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Deletes one callback key of an app.
pub async fn revoke(a: &State, t: &str, app: &str, key: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM integration_keys WHERE tenant=$1 AND app_id=$2 AND id=$3",
        t,
        app,
        key
    )
    .execute(&a.db)
    .await?;
    Ok(())
}
