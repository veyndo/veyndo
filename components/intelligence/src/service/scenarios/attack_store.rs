//! Saved attack test reports are actor-private applied task rows of the sandbox; freshness reuses the release configuration digest.
use super::evidence;
use crate::ports::AgentHost;
use crate::store::scenario_reports::{self as store, AttackReport};
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::RequestContext;
use veyndo_core::Result;
use veyndo_core::State;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
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
/// The JSON fields `save` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveInput {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    channel: Value,
}

pub async fn save(a: &State, h: &RequestContext, t: &str, report: &Value) -> Result<()> {
    let input: SaveInput = view(report);
    let actor = header(h, "x-rac-user").ok_or(bad("Scenario actor required"))?;
    let id = input.id.as_deref().ok_or(bad("Attack report required"))?;
    let mut tx = a.db.begin().await?;
    // The report is evidence, not a proposal: it is stored already applied and never queued.
    // Keep the ten most recent reports per person and sandbox.
    store::save_attack(
        &mut tx,
        t,
        AttackReport {
            id,
            proposal: &json!({"attack":report}),
            actor,
            scopes: &json!(SCOPES),
            request: &json!({"attack":{"channel":input.channel},"agentId":h.principal.agent_key}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
/// Latest saved report of this person in this sandbox and channel, with its configuration freshness.
pub async fn latest<H: AgentHost>(host: &H, h: &RequestContext, t: &str) -> Result<Value> {
    let a = host.models().core;
    let actor = header(h, "x-rac-user").ok_or(bad("Scenario actor required"))?;
    let row = store::latest_attack(
        &a.db,
        t,
        actor,
        channel_id(h),
        h.principal.agent_key.as_deref(),
    )
    .await?;
    let Some(row) = row else {
        return Ok(Value::Null);
    };
    let mut report = row.report;
    let digest = report["configurationDigest"].clone();
    let current = evidence::capture(host, t).await;
    report["savedAt"] = json!(row.saved_at);
    report["configurationReview"] = json!(evidence::review(
        &json!({"schemaVersion":1,"startedDigest":digest,"finishedDigest":digest}),
        current.as_deref(),
    ));
    Ok(report)
}
