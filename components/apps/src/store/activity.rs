//! The operator view of an app (latest calls, deliveries, storage use) and the replay of outbox events into its delivery queue.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The latest 50 calls of an app as metadata objects.
pub async fn calls(a: &State, t: &str, app: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT jsonb_build_object('id',id,'action',action,'actor',actor,'status',status,'durationMs',duration_ms,'at',created_at) AS "call!" FROM app_action_log WHERE tenant=$1 AND app=$2 ORDER BY id DESC LIMIT 50"#,
        t,
        app
    )
    .fetch_all(&a.db)
    .await?)
}

/// The latest 50 event deliveries of an app as metadata objects.
pub async fn deliveries(a: &State, t: &str, app: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT jsonb_build_object('eventId',d.event_id,'kind',o.kind,'state',d.state,'attempts',d.attempts,'error',d.error,'lastAttemptAt',d.last_attempt_at) AS "delivery!" FROM app_deliveries d JOIN outbox o ON o.tenant=d.tenant AND o.id=d.event_id WHERE d.tenant=$1 AND d.app=$2 ORDER BY d.event_id DESC LIMIT 50"#,
        t,
        app
    )
    .fetch_all(&a.db)
    .await?)
}

/// The storage use of an app with its limits, when it stored anything.
pub async fn usage(a: &State, t: &str, app: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT jsonb_build_object('rows',rows,'bytes',bytes,'rowLimit',100000,'byteLimit',67108864) AS "usage!" FROM app_storage_usage WHERE tenant=$1 AND app=$2"#,
        t,
        app
    )
    .fetch_optional(&a.db)
    .await?)
}

/// One outbox event considered for replay.
pub struct OutboxEvent {
    pub id: i64,
    pub kind: String,
    pub data: Value,
}

/// The shop's outbox events after a cursor, oldest first.
pub async fn events_after(
    conn: &mut PgConnection,
    t: &str,
    after: i64,
    limit: i64,
) -> Result<Vec<OutboxEvent>> {
    Ok(sqlx::query_as!(
        OutboxEvent,
        "SELECT id,kind,data FROM outbox WHERE tenant=$1 AND id>$2 ORDER BY id LIMIT $3",
        t,
        after,
        limit
    )
    .fetch_all(conn)
    .await?)
}

/// Queues one event for an app again; a pending or running delivery is left alone. Returns the rows queued.
pub async fn requeue(conn: &mut PgConnection, t: &str, app: &str, event: i64) -> Result<u64> {
    Ok(sqlx::query!(
        "INSERT INTO app_deliveries(tenant,app,event_id) VALUES($1,$2,$3) ON CONFLICT(tenant,app,event_id) DO UPDATE SET state='queued',attempts=0,error=NULL,available_at=now(),lease_until=NULL,lease_token=NULL WHERE app_deliveries.state IN ('failed','delivered')",
        t,
        app,
        event
    )
    .execute(conn)
    .await?
    .rows_affected())
}
