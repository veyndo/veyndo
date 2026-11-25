//! Durable sequence execution records each action before dispatch, persists delay cursors and reports uncertain effects.
use crate::ports::FlowEffects;
use crate::store::flow_access::headers;
use crate::store::flow_jobs::Claim;
use crate::store::flow_steps;
use crate::types::flow::Flow;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
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
    let context: PipelineContext = view(definition);
    let p = f.pipeline.as_ref().ok_or(bad("Pipeline required"))?;
    let graph = p.graph()?;
    let mut next = Some(job.cursor.as_deref().unwrap_or(&p.entry));
    let mut trace = view::<Execution>(&job.execution)
        .trace
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
                let data: Cart = serde_json::from_value(context.cart_data.clone())
                    .map_err(|_| bad("Missing flow cart context"))?;
                let cart = StoredCart {
                    id: String::new(),
                    tenant: t.into(),
                    token: String::new(),
                    data,
                    revision: 0,
                    status: "event".into(),
                };
                let matched = condition.checked_matches(&cart, &context.rule_context)?;
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
                let existing = flow_steps::find(a, id, node_id).await?;
                let result = if let Some(r) = existing {
                    if r.state != "completed" {
                        return Err(bad("Flow step has an uncertain previous execution"));
                    }
                    r.result.unwrap_or(Value::Null)
                } else {
                    flow_steps::start(a, id, node_id).await?;
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
                            flow_steps::complete(a, id, node_id, &v).await?;
                            v
                        }
                        Err(e) => {
                            flow_steps::fail(a, id, node_id, &e.1).await?;
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

/// The event context a queued pipeline evaluates its branches against.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PipelineContext {
    cart_data: Value,
    rule_context: Value,
}

/// The trace a pipeline checkpointed so far.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Execution {
    trace: Value,
}
