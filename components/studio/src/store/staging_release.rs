//! SQL of environment listing and release: release locks, native unit writes into the live shop, baselines and the release log.
use super::staging_documents::Tx;
use serde_json::Value;
use sqlx::Row;
use veyndo_core::{Result, State};

/// One private environment of a live shop.
pub struct Environment {
    pub tenant: String,
    pub name: String,
    pub created_at: String,
}

/// The live shop's private (non-preview) environments, newest first.
pub async fn environments(a: &State, t: &str) -> Result<Vec<Environment>> {
    let rows=sqlx::query("SELECT tenant,name,created_at::text AS created_at FROM shop_environments WHERE live_tenant=$1 AND preview_owner IS NULL ORDER BY created_at DESC").bind(t).fetch_all(&a.db).await?;
    Ok(rows
        .iter()
        .map(|r| Environment {
            tenant: r.get("tenant"),
            name: r.get("name"),
            created_at: r.get("created_at"),
        })
        .collect())
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
    let rows=sqlx::query("SELECT id,environment,selections,created_at::text AS created_at FROM shop_releases WHERE live_tenant=$1 ORDER BY created_at DESC LIMIT 30").bind(t).fetch_all(&a.db).await?;
    Ok(rows
        .iter()
        .map(|r| ReleaseRecord {
            id: r.get("id"),
            environment: r.get("environment"),
            selections: r.get("selections"),
            created_at: r.get("created_at"),
        })
        .collect())
}

/// Takes the live shop's rule and promotion write locks (726, 727) in native order.
pub async fn lock_configuration(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    for lock in [726, 727] {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,$2))")
            .bind(t)
            .bind(lock as i64)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Takes an app package fence (`<tenant>:<app>`).
pub async fn lock_package(tx: &mut Tx<'_>, key: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,7))")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Takes an app record lock.
pub async fn lock_record(tx: &mut Tx<'_>, key: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,8))")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Whether an active order is in a state outside the given workflow states.
pub async fn stranded_orders(tx: &mut Tx<'_>, t: &str, states: Vec<String>) -> Result<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orders WHERE tenant=$1 AND NOT(data->>'state'=ANY($2)) AND data->>'state' NOT IN ('expired','payment_review'))").bind(t).bind(states).fetch_one(&mut **tx).await?)
}

/// Sets whether an installed app package is active.
pub async fn set_app_active(
    tx: &mut Tx<'_>,
    t: &str,
    app: &str,
    active: Option<bool>,
) -> Result<()> {
    sqlx::query("UPDATE app_packages SET active=$1 WHERE tenant=$2 AND id=$3")
        .bind(active)
        .bind(t)
        .bind(app)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// The manifest of an installed app package.
pub async fn manifest(tx: &mut Tx<'_>, t: &str, app: &str) -> Result<Option<Value>> {
    Ok(
        sqlx::query("SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(app)
            .fetch_optional(&mut **tx)
            .await?
            .map(|r| r.get("manifest")),
    )
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
    sqlx::query("INSERT INTO commerce_rules(tenant,id,name,condition,active) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,id) DO UPDATE SET name=EXCLUDED.name,condition=EXCLUDED.condition,active=EXCLUDED.active,revision=commerce_rules.revision+1").bind(t).bind(id).bind(name).bind(condition).bind(active).execute(&mut **tx).await?;
    Ok(())
}

/// Inserts or replaces a promotion, flow or channel document (`table` is one of their fixed tables).
pub async fn upsert_unit(
    tx: &mut Tx<'_>,
    table: &str,
    t: &str,
    id: &str,
    data: &Value,
) -> Result<()> {
    let sql = format!(
        "INSERT INTO {table}(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision={table}.revision+1"
    );
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(t)
        .bind(id)
        .bind(data)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Replaces the settings or experience document (`table` is one of their fixed tables).
pub async fn replace_document(tx: &mut Tx<'_>, table: &str, t: &str, data: &Value) -> Result<()> {
    let sql = format!("UPDATE {table} SET data=$1,revision=revision+1 WHERE tenant=$2");
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(data)
        .bind(t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// A flow document.
pub async fn flow(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Value> {
    Ok(
        sqlx::query_scalar("SELECT data FROM commerce_flows WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(id)
            .fetch_one(&mut **tx)
            .await?,
    )
}

/// The environment's baseline, locked; fails when the environment does not belong to the live shop.
pub async fn baseline(tx: &mut Tx<'_>, id: &str, live: &str) -> Result<Value> {
    Ok(sqlx::query_scalar(
        "SELECT baseline FROM shop_environments WHERE tenant=$1 AND live_tenant=$2 FOR UPDATE",
    )
    .bind(id)
    .bind(live)
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
    sqlx::query("INSERT INTO shop_releases(id,live_tenant,environment,selections,actor) VALUES($1,$2,$3,$4,$5)").bind(release).bind(live).bind(environment).bind(selections).bind(actor).execute(&mut **tx).await?;
    Ok(())
}

/// The live settings document.
pub async fn settings(tx: &mut Tx<'_>, t: &str) -> Result<Value> {
    Ok(
        sqlx::query_scalar("SELECT data FROM commerce_settings WHERE tenant=$1")
            .bind(t)
            .fetch_one(&mut **tx)
            .await?,
    )
}

/// Whether a channel has its own settings overrides.
pub async fn channel_configured(tx: &mut Tx<'_>, t: &str, channel: &str) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2)",
    )
    .bind(t)
    .bind(channel)
    .fetch_one(&mut **tx)
    .await?)
}

/// A channel's settings overrides.
pub async fn channel_overrides(tx: &mut Tx<'_>, t: &str, channel: &str) -> Result<Value> {
    Ok(
        sqlx::query_scalar("SELECT data FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2")
            .bind(t)
            .bind(channel)
            .fetch_one(&mut **tx)
            .await?,
    )
}
