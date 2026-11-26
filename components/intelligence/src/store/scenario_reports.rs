//! Customer-simulation evidence in storage: saved attack reports, sandbox readiness counts and scenario groups. Columns checked at compile time.
use serde_json::Value;
use sqlx::{PgConnection, PgExecutor};
use veyndo_core::Result;

/// An attack report as it is saved: already applied evidence, never a queued proposal.
pub struct AttackReport<'a> {
    pub id: &'a str,
    pub proposal: &'a Value,
    pub actor: &'a str,
    pub scopes: &'a Value,
    pub request: &'a Value,
}

/// Saves an attack report and keeps the ten most recent reports of the person in this shop.
pub async fn save_attack(tx: &mut PgConnection, tenant: &str, r: AttackReport<'_>) -> Result<()> {
    sqlx::query!("INSERT INTO tasks(id,tenant,proposal,applied,applied_at,run_state,run_actor,run_scopes,run_request) VALUES($1,$2,$3,true,now(),'ready',$4,$5,$6)",
        r.id, tenant, r.proposal, r.actor, r.scopes, r.request).execute(&mut *tx).await?;
    sqlx::query!("DELETE FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'attack' AND id NOT IN (SELECT id FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'attack' ORDER BY created_at DESC,id LIMIT 10)",
        tenant, r.actor).execute(&mut *tx).await?;
    Ok(())
}

/// The latest saved attack report with its UTC save time.
pub struct SavedAttack {
    pub report: Value,
    pub saved_at: String,
}

/// The person's latest attack report in this channel under this agent identity.
pub async fn latest_attack(
    e: impl PgExecutor<'_>,
    tenant: &str,
    actor: &str,
    channel: &str,
    agent: Option<&str>,
) -> Result<Option<SavedAttack>> {
    Ok(sqlx::query_as!(SavedAttack, r#"SELECT proposal->'attack' AS "report!",to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "saved_at!" FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'attack' AND run_request->'attack'->>'channel'=$3 AND (run_request->>'agentId') IS NOT DISTINCT FROM $4 ORDER BY created_at DESC,id LIMIT 1"#,
        tenant, actor, channel, agent).fetch_optional(e).await?)
}

/// Purchasable units of a shop (products without variants and the variants) by state.
pub struct CatalogueCounts {
    pub products: i64,
    pub stocked: i64,
    pub empty: i64,
    pub inactive: i64,
}

/// Counts the shop's purchasable units.
pub async fn catalogue_counts(e: impl PgExecutor<'_>, tenant: &str) -> Result<CatalogueCounts> {
    Ok(sqlx::query_as!(CatalogueCounts, r#"SELECT count(*) AS "products!", count(*) FILTER (WHERE active AND stock>0) AS "stocked!", count(*) FILTER (WHERE active AND stock<=0) AS "empty!", count(*) FILTER (WHERE NOT active) AS "inactive!" FROM products p WHERE tenant=$1 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=p.tenant AND c.parent_id=p.id)"#,
        tenant).fetch_one(e).await?)
}

/// The name of a private sandbox.
pub async fn sandbox_name(e: impl PgExecutor<'_>, tenant: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(
        "SELECT name FROM shop_environments WHERE tenant=$1 AND preview_owner IS NULL",
        tenant
    )
    .fetch_one(e)
    .await?)
}

/// Scenario groups the person ran under this agent identity.
pub async fn scenario_groups(
    e: impl PgExecutor<'_>,
    tenant: &str,
    actor: &str,
    agent: Option<&str>,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(DISTINCT run_request->'scenario'->>'group') AS "count!" FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'scenario' AND (run_request->>'agentId') IS NOT DISTINCT FROM $3"#,
        tenant, actor, agent).fetch_one(e).await?)
}

/// A digest of every installed app, rule, flow, promotion and channel revision of the shop.
pub async fn extension_digest(e: impl PgExecutor<'_>, tenant: &str) -> Result<String> {
    veyndo_marketing::store::configuration_digest::digest(e, tenant).await
}
