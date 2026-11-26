//! Flow-started proposals in storage: the flow worker's lease fence, the daily review limits and the ready proposal task. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgExecutor;
use veyndo_core::Result;

/// Whether the flow job is still running under an unexpired lease, row-locked; None when the job is gone.
pub async fn lease_held(e: impl PgExecutor<'_>, tenant: &str, job: &str) -> Result<Option<bool>> {
    veyndo_marketing::store::flow_proposal_reads::lease_held(e, tenant, job).await
}

/// Proposals the job's flow created today (UTC).
pub async fn flow_proposals_today(e: impl PgExecutor<'_>, tenant: &str, job: &str) -> Result<i64> {
    veyndo_marketing::store::flow_proposal_reads::flow_proposals_today(e, tenant, job).await
}

/// Template proposals of the shop created today (UTC).
pub async fn template_proposals_today(e: impl PgExecutor<'_>, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM tasks WHERE tenant=$1 AND proposal ? 'templateSource' AND created_at>=date_trunc('day',now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'"#,
        tenant).fetch_one(e).await?)
}

/// Saves a ready proposal for review; no credential, queue state or execution.
pub async fn insert_ready(
    e: impl PgExecutor<'_>,
    id: &str,
    tenant: &str,
    evidence: &Value,
    actor: &str,
    scopes: &Value,
    request: &Value,
) -> Result<()> {
    sqlx::query!("INSERT INTO tasks(id,tenant,proposal,run_state,run_actor,run_scopes,run_request) VALUES($1,$2,$3,'ready',$4,$5,$6)",
        id, tenant, evidence, actor, scopes, request).execute(e).await?;
    Ok(())
}
