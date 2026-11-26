//! Scheduled Flow results become native reviewable tasks; only personal approval supplies execution authority.
use crate::store::flow_proposals as store;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{channel as channel_id, header};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_identity::{SCOPES, allowed};
use veyndo_marketing::store::flow_access::proposal_headers;

/// The JSON fields `persist` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PersistInput {
    instruction: Value,
    locale: Value,
    #[serde(deserialize_with = "lenient")]
    flow_template_limit: Option<u64>,
}

/// Publishes a scheduled flow's planning result as a reviewable task under the flow job's lease.
pub async fn persist(
    a: &State,
    t: &str,
    id: &str,
    evidence: &Value,
    original: &RequestContext,
) -> Result<()> {
    let input: PersistInput = view(evidence);
    let actor = header(original, "x-rac-user").ok_or(bad("Flow actor required"))?;
    let job = header(original, "x-rac-flow-job").ok_or(bad("Flow job required"))?;
    // Inference may outlive a membership/permission change. Never publish evidence
    // with the old grant or widen it to permissions gained during that call.
    let current = proposal_headers(a, t, Some(actor), job).await?;
    let scopes = SCOPES
        .iter()
        .filter(|s| allowed(original, s))
        .copied()
        .collect::<Vec<_>>();
    if scopes.iter().any(|scope| !allowed(&current, scope)) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Flow owner permissions changed during planning".into(),
        ));
    }
    let request = json!({"instruction":input.instruction,"locale":input.locale,"channel":channel_id(original),"source":"flow","sourceJobId":job});
    let mut tx = a.db.begin_privileged().await?;
    // The same live-shop lock as enqueue/approval fences completion against stop.
    let stopped: bool = veyndo_core::store::tenants::agents_stopped_locked(&mut *tx, t).await?;
    if stopped {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Shop agents are stopped by an administrator".into(),
        ));
    }
    // Serialize with recovery. A late provider response cannot publish a ready
    // task once the original worker lost its lease, even before recovery runs.
    let held = store::lease_held(&mut *tx, t, job).await?;
    if held != Some(true) {
        return Err(conflict("Flow worker lost its lease"));
    }
    if let Some(limit) = input.flow_template_limit {
        // Preparation uses no model call. Bound its saved review queue under the
        // same live-shop lock; concurrent workers cannot each spend the last slot.
        if !(1..=100).contains(&limit) {
            return Err(bad("Invalid template limit"));
        }
        let count = store::flow_proposals_today(&mut *tx, t, job).await?;
        if count >= limit as i64 {
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Flow review limit reached for today (UTC)".into(),
            ));
        }
        let pending = store::template_proposals_today(&mut *tx, t).await?;
        if pending >= 100 {
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Workspace review limit reached for today (UTC)".into(),
            ));
        }
    }
    // No credential, queued state or execution is manufactured. The existing
    // approval owner binds a real current personal session before any worker write.
    store::insert_ready(&mut *tx, id, t, evidence, actor, &json!(scopes), &request).await?;
    tx.commit().await?;
    Ok(())
}
