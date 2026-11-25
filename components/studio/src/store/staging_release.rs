//! SQL of environment listing and release: release locks, native unit writes into the live shop, baselines and the release log.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::{Result, State, bad};

/// One private environment of a live shop.
pub struct Environment {
    pub tenant: String,
    pub name: String,
    pub created_at: String,
}

/// The live shop's private (non-preview) environments, newest first.
pub async fn environments(a: &State, t: &str) -> Result<Vec<Environment>> {
    Ok(sqlx::query_as!(Environment, r#"SELECT tenant,name,created_at::text AS "created_at!" FROM shop_environments WHERE live_tenant=$1 AND preview_owner IS NULL ORDER BY created_at DESC"#, t).fetch_all(&a.db).await?)
}

/// One published release.
pub struct ReleaseRecord {
    pub id: String,
    pub environment: String,
    pub selections: Value,
    pub created_at: String,
}

/// The live shop's 30 most recent releases.
pub async fn releases(a: &State, t: &str) -> Result<Vec<ReleaseRecord>> {
    Ok(sqlx::query_as!(ReleaseRecord, r#"SELECT id,environment,selections,created_at::text AS "created_at!" FROM shop_releases WHERE live_tenant=$1 ORDER BY created_at DESC LIMIT 30"#, t).fetch_all(&a.db).await?)
}

/// Takes the live shop's rule and promotion write locks (726, 727) in native order.
pub async fn lock_configuration(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    for lock in [726, 727] {
        sqlx::query!(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,$2))",
            t,
            lock as i64
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Takes an app package fence (`<tenant>:<app>`).
pub async fn lock_package(tx: &mut Tx<'_>, key: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,7))", key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Takes an app record lock.
pub async fn lock_record(tx: &mut Tx<'_>, key: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,8))", key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Whether an active order is in a state outside the given workflow states.
pub async fn stranded_orders(tx: &mut Tx<'_>, t: &str, states: Vec<String>) -> Result<bool> {
    veyndo_orders::store::staging::stranded_orders(tx, t, states).await
}

/// Sets whether an installed app package is active.
pub async fn set_app_active(
    tx: &mut Tx<'_>,
    t: &str,
    app: &str,
    active: Option<bool>,
) -> Result<()> {
    veyndo_app_platform::store::staging::set_app_active(tx, t, app, active).await
}

/// The manifest of an installed app package.
pub async fn manifest(tx: &mut Tx<'_>, t: &str, app: &str) -> Result<Option<Value>> {
    veyndo_app_platform::store::staging::manifest(tx, t, app).await
}

/// Inserts or replaces a rule.
pub async fn upsert_rule(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    name: &Value,
    condition: &Value,
    active: Option<bool>,
) -> Result<()> {
    veyndo_marketing::store::staging::upsert_rule(tx, t, id, name, condition, active).await
}

/// Inserts or replaces a promotion, flow or channel document (`table` is one of their fixed tables).
pub async fn upsert_unit(
    tx: &mut Tx<'_>,
    table: &str,
    t: &str,
    id: &str,
    data: &Value,
) -> Result<()> {
    veyndo_marketing::store::staging::upsert_unit(tx, table, t, id, data).await
}

/// Replaces the settings or experience document (`table` is one of their fixed tables).
pub async fn replace_document(tx: &mut Tx<'_>, table: &str, t: &str, data: &Value) -> Result<()> {
    match table {
        "commerce_settings" => veyndo_shop::store::staging::replace_settings(tx, t, data).await,
        "experiences" => veyndo_intelligence::store::staging::replace_experience(tx, t, data).await,
        _ => Err(bad("Unsupported release unit")),
    }
}

/// A flow document.
pub async fn flow(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Value> {
    veyndo_marketing::store::staging::flow(tx, t, id).await
}

/// The environment's baseline, locked; fails when the environment does not belong to the live shop.
pub async fn baseline(tx: &mut Tx<'_>, id: &str, live: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT baseline FROM shop_environments WHERE tenant=$1 AND live_tenant=$2 FOR UPDATE",
        id,
        live
    )
    .fetch_one(&mut **tx)
    .await?)
}

/// Records a published release.
pub async fn insert_release(
    tx: &mut Tx<'_>,
    release: &str,
    live: &str,
    environment: &str,
    selections: &Value,
    actor: &str,
) -> Result<()> {
    sqlx::query!("INSERT INTO shop_releases(id,live_tenant,environment,selections,actor) VALUES($1,$2,$3,$4,$5)", release, live, environment, selections, actor).execute(&mut **tx).await?;
    Ok(())
}

/// The live settings document.
pub async fn settings(tx: &mut Tx<'_>, t: &str) -> Result<Value> {
    veyndo_shop::store::staging::settings(tx, t).await
}

/// Whether a channel has its own settings overrides.
pub async fn channel_configured(tx: &mut Tx<'_>, t: &str, channel: &str) -> Result<bool> {
    veyndo_shop::store::staging::channel_configured(tx, t, channel).await
}

/// A channel's settings overrides.
pub async fn channel_overrides(tx: &mut Tx<'_>, t: &str, channel: &str) -> Result<Value> {
    veyndo_shop::store::staging::channel_overrides(tx, t, channel).await
}
