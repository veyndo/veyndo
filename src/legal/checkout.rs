//! Explicit document/digital acknowledgement and immutable order policy snapshots; transport-neutral checkout guard.
use super::*;
use model::version;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Acceptance {
    policy_version: String,
    terms: bool,
    digital_immediate: bool,
}
pub(super) async fn accept(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Acceptance>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let mut tx = a.db.begin().await?;
    let (s, _) = commerce::scoped_locked(&mut tx, &c.tenant, &c.data.sales_channel).await?;
    if v.policy_version != version(&s.legal) {
        return Err(conflict("Legal documents changed; review again"));
    }
    let mut data = json!({"policyVersion":v.policy_version,"terms":v.terms,"digitalImmediate":v.digital_immediate,"locale":c.data.locale,"recordedAt":chrono::Utc::now().to_rfc3339()});
    if let Some(actor) = &h.admin_order {
        data["source"] = json!("merchantRecordedCustomerAcknowledgement");
        data["actor"] = json!(actor);
    }
    sqlx::query("INSERT INTO legal_acceptances(tenant,cart_id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,cart_id) DO UPDATE SET data=EXCLUDED.data")
      .bind(&c.tenant).bind(&c.id).bind(&data).execute(&mut *tx).await?;
    agent_runs::checkpoint::fence_commit(&a, &mut tx).await?;
    tx.commit().await?;
    Ok(Json(data))
}
pub(crate) use veyndo_checkout::store::legal_snapshot::snapshot;
