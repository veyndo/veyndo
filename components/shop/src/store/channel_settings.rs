//! Settings, channel overrides and hosted frontend bindings as other components read them inside their own transactions: single fields, documents and usage counts of one channel. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The shop's main locale as stored in its settings.
pub async fn main_locale(conn: &mut PgConnection, tenant: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        r#"SELECT data->'mainLocale' AS "locale!: Value" FROM commerce_settings WHERE tenant=$1"#,
        tenant
    )
    .fetch_one(conn)
    .await?)
}

/// Locks the shop settings row so a channel save and checkout see the same settings.
pub async fn lock_basis(conn: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query_scalar!(
        "SELECT revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        tenant
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Inserts the first overrides of a new channel; an existing row is an error.
pub async fn insert_override(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO commerce_overrides(tenant,channel_id,data) VALUES($1,$2,$3)",
        tenant,
        channel,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// A settings document with the scope it belongs to: `default` for the basis, the channel id for an override.
pub struct ScopedDocument {
    pub id: String,
    pub data: Value,
}

/// The basis settings document of the shop, as rows (none for an unknown shop).
pub async fn basis_documents(conn: &mut PgConnection, tenant: &str) -> Result<Vec<ScopedDocument>> {
    Ok(sqlx::query_as!(
        ScopedDocument,
        r#"SELECT 'default' AS "id!",data FROM commerce_settings WHERE tenant=$1"#,
        tenant
    )
    .fetch_all(conn)
    .await?)
}

/// Every channel override document of the shop, unlocked.
pub async fn override_documents(
    conn: &mut PgConnection,
    tenant: &str,
) -> Result<Vec<ScopedDocument>> {
    Ok(sqlx::query_as!(
        ScopedDocument,
        "SELECT channel_id AS id,data FROM commerce_overrides WHERE tenant=$1",
        tenant
    )
    .fetch_all(conn)
    .await?)
}

/// How many settings overrides channel `channel` has (zero or one).
pub async fn override_count(conn: &mut PgConnection, tenant: &str, channel: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2"#,
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}

/// How many per-channel company master data rows channel `channel` has.
pub async fn company_override_count(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM company_overrides WHERE tenant=$1 AND channel_id=$2"#,
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}

/// How many hosted frontends serve channel `channel`.
pub async fn frontend_count(conn: &mut PgConnection, tenant: &str, channel: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM hosted_frontends WHERE tenant=$1 AND channel=$2"#,
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}

/// Whether the hosted frontend `alias` serves channel `channel`.
pub async fn frontend_serves(a: &State, tenant: &str, channel: &str, alias: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE tenant=$1 AND channel=$2 AND alias=$3) AS "own!""#,
        tenant,
        channel,
        alias
    )
    .fetch_one(&a.db)
    .await?)
}
