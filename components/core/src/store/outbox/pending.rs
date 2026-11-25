//! Counts of undelivered outbox events for the operator views.
use crate::Result;
use sqlx::PgConnection;

/// Undelivered events of all shops.
pub async fn all<'e>(db: impl sqlx::PgExecutor<'e>) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) FILTER(WHERE delivered_at IS NULL) AS "pending!" FROM outbox"#
    )
    .fetch_one(db)
    .await?)
}

/// Undelivered events of all shops outside `excluded`.
pub async fn outside(conn: &mut PgConnection, excluded: &[String]) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM outbox WHERE delivered_at IS NULL AND NOT (tenant=ANY($1))"#,
        excluded
    )
    .fetch_one(conn)
    .await?)
}
