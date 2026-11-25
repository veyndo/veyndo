//! Age of the oldest due flow job, for the app worker's queue diagnostics.
use veyndo_core::Result;

/// Seconds since the oldest due flow job became available; `None` when none is due.
pub async fn oldest_due_flow<'e>(db: impl sqlx::PgExecutor<'e>) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT
      (SELECT floor(extract(epoch FROM now() - available_at))::bigint
       FROM flow_jobs WHERE state='queued' AND available_at<=now()
       ORDER BY available_at LIMIT 1) AS "flow?""#
    )
    .fetch_one(db)
    .await?)
}
