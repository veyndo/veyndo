//! Flow job state that flow-template proposals check: whether the job still holds its lease and how many proposals a job made today.
use sqlx::PgExecutor;
use veyndo_core::Result;

/// Whether the flow job is still running under an unexpired lease, row-locked; None when the job is gone.
pub async fn lease_held(e: impl PgExecutor<'_>, tenant: &str, job: &str) -> Result<Option<bool>> {
    Ok(sqlx::query_scalar!(r#"WITH held AS MATERIALIZED (SELECT state,lease_until FROM flow_jobs WHERE tenant=$1 AND id=$2 FOR UPDATE) SELECT state='running' AND lease_until>clock_timestamp() AS "held!" FROM held"#,
        tenant, job).fetch_optional(e).await?)
}

/// Proposals the job's flow created today (UTC).
pub async fn flow_proposals_today(e: impl PgExecutor<'_>, tenant: &str, job: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM tasks task JOIN flow_jobs source ON source.tenant=task.tenant AND source.id=task.run_request->>'sourceJobId' WHERE task.tenant=$1 AND source.flow=(SELECT flow FROM flow_jobs WHERE tenant=$1 AND id=$2) AND task.created_at>=date_trunc('day',now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'"#,
        tenant, job).fetch_one(e).await?)
}
