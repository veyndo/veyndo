//! Saved attack test reports are actor-private applied task rows of the sandbox; freshness reuses the release configuration digest.
use super::evidence;
use crate::ports::AgentHost;
use serde_json::Value;
use serde_json::json;
use sqlx::Row;
use veyndo_core::RequestContext;
use veyndo_core::Result;
use veyndo_core::State;
use veyndo_core::bad;
use veyndo_core::shop_scope::channel as channel_id;
use veyndo_core::shop_scope::header;
const SCOPES: [&str; 7] = [
    "assistant.use",
    "catalog.read",
    "channels.read",
    "environments.manage",
    "orders.write",
    "orders.read",
    "knowledge.read",
];
pub async fn save(a: &State, h: &RequestContext, t: &str, report: &Value) -> Result<()> {
    let actor = header(h, "x-rac-user").ok_or(bad("Scenario actor required"))?;
    let id = report["id"].as_str().ok_or(bad("Attack report required"))?;
    let mut tx = a.db.begin().await?;
    // The report is evidence, not a proposal: it is stored already applied and never queued.
    sqlx::query("INSERT INTO tasks(id,tenant,proposal,applied,applied_at,run_state,run_actor,run_scopes,run_request) VALUES($1,$2,$3,true,now(),'ready',$4,$5,$6)")
        .bind(id).bind(t).bind(json!({"attack":report})).bind(actor).bind(json!(SCOPES))
        .bind(json!({"attack":{"channel":report["channel"]},"agentId":h.principal.agent_key}))
        .execute(&mut *tx).await?;
    // Keep the ten most recent reports per person and sandbox.
    sqlx::query("DELETE FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'attack' AND id NOT IN (SELECT id FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'attack' ORDER BY created_at DESC,id LIMIT 10)")
        .bind(t).bind(actor).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
/// Latest saved report of this person in this sandbox and channel, with its configuration freshness.
pub async fn latest<H: AgentHost>(host: &H, h: &RequestContext, t: &str) -> Result<Value> {
    let a = host.models().core;
    let actor = header(h, "x-rac-user").ok_or(bad("Scenario actor required"))?;
    let row = sqlx::query("SELECT proposal->'attack' AS report,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS saved_at FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_request ? 'attack' AND run_request->'attack'->>'channel'=$3 AND (run_request->>'agentId') IS NOT DISTINCT FROM $4 ORDER BY created_at DESC,id LIMIT 1")
        .bind(t).bind(actor).bind(channel_id(h)).bind(&h.principal.agent_key).fetch_optional(&a.db).await?;
    let Some(row) = row else {
        return Ok(Value::Null);
    };
    let mut report: Value = row.get("report");
    let digest = report["configurationDigest"].clone();
    let current = evidence::capture(host, t).await;
    report["savedAt"] = json!(row.get::<String, _>("saved_at"));
    report["configurationReview"] = json!(evidence::review(
        &json!({"schemaVersion":1,"startedDigest":digest,"finishedDigest":digest}),
        current.as_deref(),
    ));
    Ok(report)
}
