//! Durable sequence execution records each action before dispatch, persists delay cursors and reports uncertain effects.
use crate::ports::FlowEffects;
use crate::store::flow_access::headers;
use crate::store::flow_jobs::Claim;
use crate::types::flow::Flow;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Result, State, bad};
use veyndo_flow_graph::execution::Directive;
use veyndo_shop::types::cart::{Cart, StoredCart};

/// Where a pipeline run stopped.
pub enum Outcome {
    /// The pipeline finished or stopped, with its trace.
    Finished(Value),
    /// A delay node deferred the rest of the run.
    Deferred,
}

/// Runs the pipeline from the job's cursor: every action is recorded before dispatch and an uncertain step is never repeated.
pub async fn run(a: &State, effects: &dyn FlowEffects, job: &Claim, f: &Flow) -> Result<Outcome> {
    let (t, id, definition) = (&job.tenant, &job.id, &job.definition);
    let p = f.pipeline.as_ref().ok_or(bad("Pipeline required"))?;
    let graph = p.graph()?;
    let mut next = Some(job.cursor.as_deref().unwrap_or(&p.entry));
    let mut trace = job.execution["trace"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    while let Some(node_id) = next {
        job.renew(a).await?;
        headers(a, t, f.actor.as_deref()).await?;
        match graph.directive(node_id).ok_or(bad("Unknown flow cursor"))? {
            Directive::Branch {
                condition,
                branches,
            } => {
                let data: Cart = serde_json::from_value(definition["cartData"].clone())
                    .map_err(|_| bad("Missing flow cart context"))?;
                let cart = StoredCart {
                    id: String::new(),
                    tenant: t.into(),
                    token: String::new(),
                    data,
                    revision: 0,
                    status: "event".into(),
                };
                let matched = condition.checked_matches(&cart, &definition["ruleContext"])?;
                next = branches.select(matched);
                trace.push(json!({"node":node_id,"matched":matched}));
            }
            Directive::Stop => {
                return Ok(Outcome::Finished(json!({"stopped":true,"trace":trace})));
            }
            Directive::Defer {
                seconds,
                next: target,
            } => {
                job.defer(a, target, seconds, json!({"trace":trace,"delayCompleted":node_id,"finishAfterDelay":target.is_none()})).await?;
                return Ok(Outcome::Deferred);
            }
            Directive::Effect {
                action,
                config,
                next: target,
            } => {
                if action == "action.stop.flow" {
                    return Ok(Outcome::Finished(json!({"stopped":true,"trace":trace})));
                }
                let existing =
                    sqlx::query("SELECT state,result FROM flow_steps WHERE job=$1 AND node=$2")
                        .bind(id)
                        .bind(node_id)
                        .fetch_optional(&a.db)
                        .await?;
                let result = if let Some(r) = existing {
                    if r.get::<String, _>("state") != "completed" {
                        return Err(bad("Flow step has an uncertain previous execution"));
                    }
                    r.get::<Value, _>("result")
                } else {
                    sqlx::query("INSERT INTO flow_steps(job,node) VALUES($1,$2)")
                        .bind(id)
                        .bind(node_id)
                        .execute(&a.db)
                        .await?;
                    let result = effects
                        .execute(
                            t,
                            f,
                            &format!("flow:{id}:{node_id}"),
                            action,
                            config,
                            definition,
                        )
                        .await;
                    match result {
                        Ok(v) => {
                            sqlx::query("UPDATE flow_steps SET state='completed',result=$1,finished_at=now() WHERE job=$2 AND node=$3").bind(&v).bind(id).bind(node_id).execute(&a.db).await?;
                            v
                        }
                        Err(e) => {
                            sqlx::query("UPDATE flow_steps SET state='failed',error=$1,finished_at=now() WHERE job=$2 AND node=$3").bind(&e.1).bind(id).bind(node_id).execute(&a.db).await?;
                            return Err(e);
                        }
                    }
                };
                trace.push(json!({"node":node_id,"action":action,"result":result}));
                next = target;
            }
        }
        job.checkpoint(a, next, json!({"trace":trace})).await?;
    }
    Ok(Outcome::Finished(json!({"completed":true,"trace":trace})))
}
