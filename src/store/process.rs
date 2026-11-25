//! Process-level statements: the readiness ping and the short lock wait of a background digest.
use sqlx::PgConnection;
use veyndo_core::Result;

/// A round trip to the database for the health endpoint.
pub(crate) async fn ping<'e>(db: impl sqlx::PgExecutor<'e>) -> Result<()> {
    sqlx::query_scalar!("SELECT 1").fetch_one(db).await?;
    Ok(())
}

/// Lets a background read give up after 250 ms instead of queueing behind writers.
pub(crate) async fn short_lock_timeout(conn: &mut PgConnection) -> Result<()> {
    sqlx::query!("SET LOCAL lock_timeout='250ms'")
        .execute(conn)
        .await?;
    Ok(())
}

/// Seconds since the oldest due flow job and app delivery became available; `None` when none is due.
pub(crate) async fn oldest_due<'e>(
    db: impl sqlx::PgExecutor<'e>,
) -> Result<(Option<i64>, Option<i64>)> {
    let r = sqlx::query!(
        r#"SELECT
      (SELECT floor(extract(epoch FROM now() - available_at))::bigint
       FROM flow_jobs WHERE state='queued' AND available_at<=now()
       ORDER BY available_at LIMIT 1) AS "flow?",
      (SELECT floor(extract(epoch FROM now() - available_at))::bigint
       FROM app_deliveries WHERE state='queued' AND available_at<=now()
       ORDER BY available_at LIMIT 1) AS "delivery?""#
    )
    .fetch_one(db)
    .await?;
    Ok((r.flow, r.delivery))
}
