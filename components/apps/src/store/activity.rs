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

/// The latest 50 event deliveries of an app as metadata objects. Every delivery references its outbox
/// event (foreign key), so the event kinds come from the outbox owner for exactly these deliveries.
pub async fn deliveries(a: &State, t: &str, app: &str) -> Result<Vec<Value>> {
    let rows = sqlx::query!(
        r#"SELECT event_id,state,attempts,error,jsonb_build_object('lastAttemptAt',last_attempt_at) AS "at!" FROM app_deliveries WHERE tenant=$1 AND app=$2 ORDER BY event_id DESC LIMIT 50"#,
        t,
        app
    )
    .fetch_all(&a.db)
    .await?;
    let ids: Vec<i64> = rows.iter().map(|r| r.event_id).collect();
    let kinds: std::collections::HashMap<i64, String> =
        veyndo_core::store::outbox_reads::kinds(&a.db, t, &ids)
            .await?
            .into_iter()
            .map(|k| (k.id, k.kind))
            .collect();
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let kind = kinds.get(&r.event_id)?;
            Some(serde_json::json!({"eventId":r.event_id,"kind":kind,"state":r.state,"attempts":r.attempts,"error":r.error,"lastAttemptAt":r.at.get("lastAttemptAt")}))
        })
        .collect())
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

pub use veyndo_core::store::outbox_reads::{Event as OutboxEvent, after as events_after};

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
