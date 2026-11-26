//! Process-level statements: the readiness ping, the short lock wait of a background digest and the queue ages of the app worker sample.
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

/// Seconds since the oldest due flow job (marketing) and app delivery (apps) became available; `None` when none is due.
pub(crate) async fn oldest_due<'e, E: sqlx::PgExecutor<'e> + Copy>(
    db: E,
) -> Result<(Option<i64>, Option<i64>)> {
    let flow = veyndo_marketing::store::queue_health::oldest_due_flow(db).await?;
    let delivery = veyndo_app_platform::store::queue_health::oldest_due_delivery(db).await?;
    Ok((flow, delivery))
}
