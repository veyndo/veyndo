//! The run worker executes claimed runs over the server's tools; claims, recovery and failure records live in the intelligence component.
use super::*;
use tracing::Instrument;
use veyndo_intelligence::store::agent_runs as store;
pub(crate) async fn once(a: &App) -> Result<bool> {
    // A dead worker can be resumed safely between calls; an in-flight provider result is ambiguous.
    store::recover(a).await?;
    let lease = uid();
    let Some(row) = store::claim(a, &lease).await? else {
        return Ok(false);
    };
    let t: String = row.get("tenant");
    let id: String = row.get("id");
    let request: Value = row.get("run_request");
    let trace: String = row.get("run_trace");
    let result = veyndo::trace::scoped(
        trace.clone(),
        async {
            let h = authority::headers(a, &row).await?;
            let model_authority = super::controls::model_authority(&h);
            let scope = if h.principal.personal_session {
                veyndo::tenant_scope::Scope::Personal {
                    tenant: t.clone(),
                    actor: row.get("run_actor"),
                }
            } else {
                veyndo::tenant_scope::Scope::Actor {
                    tenant: t.clone(),
                    actor: row.get("run_actor"),
                }
            };
            let run = Run {
                tenant: t.clone(),
                id: id.clone(),
                lease: lease.clone(),
                workspace: request["workspaceContext"].clone(),
                conversation: row.get::<Option<String>, _>("conversation_id").is_some(),
            };
            veyndo::tenant_scope::scoped(
                scope,
                veyndo::inference::budget::scoped(
                    model_authority,
                    CURRENT.scope(run, async {
                        if request["scenario"].is_object() {
                            return Box::pin(scenarios::execute(a, &t, &id, &h, &request)).await;
                        }
                        if request["execution"].is_array() {
                            return Box::pin(work_plan::execute(a, &t, &id, &h, &request)).await;
                        }
                        let choice = veyndo_intelligence::service::models::choice(&request)?;
                        let turn = veyndo_intelligence::service::planner::Turn {
                            instruction: request["instruction"].as_str().unwrap_or(""),
                            choice: choice.as_ref(),
                            history: request["history"].as_str().unwrap_or(""),
                            locale: request["locale"].as_str().unwrap_or("en-GB"),
                        };
                        tokio::time::timeout(
                            std::time::Duration::from_secs(190),
                            Box::pin(veyndo_intelligence::service::planner::plan_with(
                                &crate::agent_host::Host(a),
                                &t,
                                turn,
                                &h,
                            )),
                        )
                        .await
                        .unwrap_or_else(|_| {
                            Err(Error(
                                StatusCode::GATEWAY_TIMEOUT,
                                "Model turn timed out".into(),
                            ))
                        })
                    }),
                ),
            )
            .await
        }
        .instrument(tracing::info_span!("agent_run",trace_id=%trace,task_id=%id)),
    )
    .await;
    if let Err(error) = result {
        let run = store::Lease {
            tenant: &t,
            id: &id,
            lease: &lease,
        };
        store::fail(a, run, &error).await?;
    }
    Ok(true)
}
