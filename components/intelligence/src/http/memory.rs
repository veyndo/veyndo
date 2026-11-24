//! Merchant memory reads and revision-bound hypothesis decisions.
use crate::store::observations::observations;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::{merchant, permit};

/// The shop's evidence-based memory: observed pairs and reviewable hypotheses.
pub async fn memory(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    permit(&h, "knowledge.read")?;
    Ok(Json(observations(&a, &merchant(&h)?).await?))
}
/// Dismisses, marks for experiment or publishes a hypothesis at its revision; it never starts an experiment.
pub async fn decide(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    permit(&h, "catalog")?;
    let t = merchant(&h)?;
    let state = v["state"]
        .as_str()
        .filter(|v| ["dismissed", "experiment", "published"].contains(v))
        .ok_or(bad("Choose dismissed, experiment or published"))?;
    if v["approve"] != true {
        return Err(bad("Explicit approve=true required"));
    }
    let mut tx = a.db.begin().await?;
    let changed=sqlx::query("UPDATE knowledge_hypotheses SET state=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4").bind(state).bind(&t).bind(&id).bind(v["revision"].as_i64().ok_or(bad("revision required"))?).execute(&mut *tx).await?.rows_affected();
    if changed != 1 {
        return Err(conflict("Hypothesis revision changed"));
    }
    veyndo_core::outbox::publish_on(
        &mut *tx,
        &t,
        "intelligence.decision",
        &json!({"hypothesisId":id,"state":state,"experimentStarted":false}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"state":state,"experimentStarted":false,"nextStep":"Configure a controlled experiment; marking an idea does not run one"}),
    ))
}
