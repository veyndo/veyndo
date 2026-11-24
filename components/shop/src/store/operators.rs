//! Platform operator grants and the control-plane database probe.
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Result, State};

/// Whether the user holds an operator grant, and whether it is active; none without a grant.
pub async fn grant(a: &State, user: &str) -> Result<Option<bool>> {
    Ok(
        sqlx::query_scalar("SELECT active FROM platform_operators WHERE user_id=$1")
            .bind(user)
            .fetch_optional(&a.db)
            .await?,
    )
}

/// Database size, version, active connections, commits and the undelivered outbox events.
pub async fn database_probe(a: &State) -> Result<(Value, i64)> {
    let database=sqlx::query("SELECT pg_database_size(current_database()) AS bytes,version() AS version,(SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND state='active') AS active_connections,(SELECT sum(xact_commit)::text FROM pg_stat_database WHERE datname=current_database()) AS commits").fetch_one(&a.db).await?;
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FILTER(WHERE delivered_at IS NULL) FROM outbox")
            .fetch_one(&a.db)
            .await?;
    Ok((
        json!({"bytes":database.get::<i64,_>("bytes"),"version":database.get::<String,_>("version"),"activeConnections":database.get::<i64,_>("active_connections"),"commits":database.get::<Option<String>,_>("commits")}),
        pending,
    ))
}
