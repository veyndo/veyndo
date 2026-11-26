//! Bounded journal of prepared context and completed inference calls; ambiguous calls require explicit continuation.
use super::agent_controls::model_authority;
use super::agent_runs::CURRENT;
use super::planner::validate_context;
use crate::domain::tool_trace::attachment_references;
use crate::ports::AgentHost;
use crate::store::agent_runs as store;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_cognition::inference::{Choice, Output, budget};
use veyndo_core::{Error, RequestContext, Result, State, bad, conflict, hash};
use veyndo_identity::assistant_access;
use veyndo_identity::store::agent_authority::{headers, headers_on};
/// Joins a native mutation's commit to its durable run. A cancellation that wins
/// while commerce waits on another row rolls the entire mutation back; a later
/// cancellation waits for this admitted commit rather than accepting a late write.
pub async fn fence_commit(a: &State, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<()> {
    let Some(run) = CURRENT.try_with(Clone::clone).ok() else {
        return Ok(());
    };
    let row = store::running_locked(tx, lease(&run))
        .await?
        .ok_or(conflict("Agent run stopped before commerce commit"))?;
    if a.db.has_separate_authority() {
        // The signed runtime has a separate, restricted system pool. Never
        // reread this task through the tenant pool while its tx is held.
        headers(a, &row).await?;
    } else {
        // Legacy development uses one pool. Reuse the connection under the
        // native owner's legacy scope; signed tickets are never changed here.
        let previous = store::system_setting(tx).await?;
        store::set_system_setting(tx, "on").await?;
        let result = headers_on(&mut *tx, &row).await;
        store::set_system_setting(tx, &previous).await?;
        result?;
    }
    // Row-lock and authority waits may cross the lease deadline. A fresh
    // statement checks time after those waits while we still own the task lock.
    let current = store::lease_current(tx, &run.tenant, &run.id).await?;
    if !current {
        return Err(conflict("Agent lease expired before commerce commit"));
    }
    Ok(())
}
/// The lease of a run.
fn lease(run: &super::agent_runs::Run) -> store::Lease<'_> {
    store::Lease {
        tenant: &run.tenant,
        id: &run.id,
        lease: &run.lease,
    }
}
/// Rechecks the running lease, the saved credential and the saved context's sources.
pub async fn authorize<H: AgentHost>(host: &H) -> Result<()> {
    let a = host.models().core;
    let Some(run) = CURRENT.try_with(Clone::clone).ok() else {
        return Ok(());
    };
    let row = store::running(a, lease(&run))
        .await?
        .ok_or(conflict("Agent run cancelled or lease lost"))?;
    let h = headers(a, &row).await?;
    let saved: Value = row.get("run_checkpoint");
    if let Some(context) = saved.get("context") {
        validate_context(
            host,
            &run.tenant,
            &h,
            &row.get::<Value, _>("run_request"),
            context,
        )
        .await?;
    }
    Ok(())
}
/// A completed checkpoint of the running task.
pub async fn load<H: AgentHost>(host: &H, key: &str) -> Result<Option<Value>> {
    let a = host.models().core;
    let Some(run) = CURRENT.try_with(Clone::clone).ok() else {
        return Ok(None);
    };
    authorize(host).await?;
    store::load(
        a,
        store::Lease {
            tenant: &run.tenant,
            id: &run.id,
            lease: &run.lease,
        },
        key,
    )
    .await
}
/// Saves the app views of a conversation run's trace for its progress.
pub async fn app_views<H: AgentHost>(host: &H, trace: &[Value]) -> Result<()> {
    if CURRENT.try_with(|run| run.conversation).unwrap_or(false) {
        let views = attachment_references(trace);
        save(host, "app-views", &json!(views)).await?;
    }
    Ok(())
}
/// Saves a checkpoint of the running task within its bounded budget.
pub async fn save<H: AgentHost>(host: &H, key: &str, value: &Value) -> Result<()> {
    let a = host.models().core;
    let Some(run) = CURRENT.try_with(Clone::clone).ok() else {
        return Ok(());
    };
    if value.to_string().len() > 524288 {
        return Err(bad("Agent checkpoint exceeds its bounded budget"));
    }
    authorize(host).await?;
    store::save(
        a,
        store::Lease {
            tenant: &run.tenant,
            id: &run.id,
            lease: &run.lease,
        },
        key,
        value,
    )
    .await
}
/// One journaled model call: a completed result is replayed, an unknown outcome stays pending.
#[tracing::instrument(skip_all,fields(trace_id=%veyndo_runtime::trace::current()))]
pub async fn model<H: AgentHost>(
    host: &H,
    h: &RequestContext,
    choice: Option<&Choice>,
    system: &str,
    prompt: &str,
    schema: &Value,
    public: bool,
) -> Result<Output> {
    let models = host.models();
    let a = models.core;
    let key = format!(
        "model-{}",
        hash(&json!([system, prompt, schema, choice]).to_string())
    );
    if let Some(value) = load(host, &key).await? {
        return serde_json::from_value(value).map_err(|_| bad("Invalid saved model checkpoint"));
    }
    if let Ok(run) = CURRENT.try_with(Clone::clone) {
        authorize(host).await?;
        store::pending(
            a,
            store::Lease {
                tenant: &run.tenant,
                id: &run.id,
                lease: &run.lease,
            },
            &key,
        )
        .await?;
    }
    assistant_access(h).or_else(|e| if public { Ok(()) } else { Err(e) })?;
    let authority = model_authority(h);
    let output = budget::scoped(
        authority,
        Box::pin(models.inference.structured_for(
            if public { "concierge" } else { "planner" },
            choice,
            system,
            prompt,
            schema,
        )),
    )
    .await
    .map_err(Error::from)?;
    save(host, &key, &json!(output)).await?;
    Ok(output)
}
/// Publishes a finished proposal under the run lease, or as a flow proposal outside a run.
pub async fn persist<H: AgentHost>(
    host: &H,
    t: &str,
    id: &str,
    evidence: &Value,
    h: &RequestContext,
) -> Result<()> {
    let a = host.models().core;
    let Some(run) = CURRENT.try_with(Clone::clone).ok() else {
        return super::flow_proposals::persist(a, t, id, evidence, h).await;
    };
    authorize(host).await?;
    store::publish(a, t, id, evidence, &run.lease).await
}
