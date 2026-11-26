//! Age of the oldest due app event delivery, for the worker's queue diagnostics.
use veyndo_core::Result;

/// Seconds since the oldest due app delivery became available; `None` when none is due.
pub async fn oldest_due_delivery<'e>(db: impl sqlx::PgExecutor<'e>) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT
      (SELECT floor(extract(epoch FROM now() - available_at))::bigint
       FROM app_deliveries WHERE state='queued' AND available_at<=now()
       ORDER BY available_at LIMIT 1) AS "delivery?""#
    )
    .fetch_one(db)
    .await?)
}
