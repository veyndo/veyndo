//! Outbox events become queued deliveries for every active package that may receive them.
use crate::domain::event_projection;
use serde_json::Value;
use sqlx::Row;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad};

/// Queues one delivery per active package that is eligible for an outbox event.
pub async fn project_events(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: i64,
    kind: &str,
) -> Result<()> {
    let rows = sqlx::query("SELECT manifest FROM app_packages WHERE tenant=$1 AND active")
        .bind(t)
        .fetch_all(&mut **tx)
        .await?;
    let data: Value = sqlx::query_scalar("SELECT data FROM outbox WHERE tenant=$1 AND id=$2")
        .bind(t)
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    for row in rows {
        let m: Manifest = serde_json::from_value(row.get("manifest"))
            .map_err(|_| bad("Invalid app event package"))?;
        if event_projection::eligible(&m, kind, &data) {
            sqlx::query("INSERT INTO app_deliveries(tenant,app,event_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(t).bind(&m.id).bind(id).execute(&mut **tx).await?;
        }
    }

    Ok(())
}

/// The delivery a worker leased: its shop, app and first event.
pub struct Claim {
    pub tenant: String,
    pub app: String,
    pub event_id: i64,
}

/// Leases the next due delivery of an active live shop for 30 seconds: configured services or public webhooks only, at most two running leases per shop, least recently served app first.
pub async fn claim_next(
    conn: &mut sqlx::PgConnection,
    services: &[String],
    lease: uuid::Uuid,
) -> Result<Option<Claim>> {
    Ok(sqlx::query_as!(
        Claim,
        "WITH candidate AS (SELECT d.tenant,d.app,d.event_id FROM app_deliveries d JOIN app_packages p ON p.tenant=d.tenant AND p.id=d.app AND p.active JOIN tenants owner ON owner.id=d.tenant AND owner.status='active' WHERE NOT EXISTS(SELECT 1 FROM shop_environments s WHERE s.tenant=d.tenant) AND (d.app=ANY($1) OR p.manifest#>>'{eventDelivery,url}' IS NOT NULL) AND d.available_at<=now() AND (d.state='queued' OR d.state='running' AND d.lease_until<now()) AND (SELECT count(DISTINCT lease_token) FROM app_deliveries busy WHERE busy.tenant=d.tenant AND busy.state='running' AND busy.lease_until>now())<2 ORDER BY (SELECT max(last_attempt_at) FROM app_deliveries previous WHERE previous.tenant=d.tenant AND previous.app=d.app) NULLS FIRST,d.event_id LIMIT 1 FOR UPDATE OF d,owner SKIP LOCKED) UPDATE app_deliveries d SET state='running',attempts=attempts+1,lease_token=$2,last_attempt_at=now(),lease_until=now()+interval '30 seconds' FROM candidate c WHERE d.tenant=c.tenant AND d.app=c.app AND d.event_id=c.event_id RETURNING d.tenant,d.app,d.event_id",
        services,
        lease
    )
    .fetch_optional(conn)
    .await?)
}

/// Adds up to `extra` further due deliveries of the same app to a lease, for a batched send.
pub async fn claim_more(
    conn: &mut sqlx::PgConnection,
    t: &str,
    app: &str,
    lease: uuid::Uuid,
    extra: i64,
) -> Result<()> {
    sqlx::query!(
        "WITH candidates AS (SELECT tenant,app,event_id FROM app_deliveries WHERE tenant=$1 AND app=$2 AND available_at<=now() AND (state='queued' OR state='running' AND lease_until<now()) ORDER BY event_id LIMIT $4 FOR UPDATE SKIP LOCKED) UPDATE app_deliveries d SET state='running',attempts=attempts+1,lease_token=$3,last_attempt_at=now(),lease_until=now()+interval '30 seconds' FROM candidates c WHERE d.tenant=c.tenant AND d.app=c.app AND d.event_id=c.event_id",
        t,
        app,
        lease,
        extra
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// One leased event with its delivery attempt count.
pub struct LeasedEvent {
    pub id: i64,
    pub kind: String,
    pub data: Value,
    pub trace_id: String,
    pub attempts: i32,
}

/// The outbox events of a lease, oldest first.
pub async fn leased_events(
    conn: &mut sqlx::PgConnection,
    t: &str,
    app: &str,
    lease: uuid::Uuid,
) -> Result<Vec<LeasedEvent>> {
    Ok(sqlx::query_as!(
        LeasedEvent,
        "SELECT o.id,o.kind,o.data,o.trace_id,d.attempts FROM app_deliveries d JOIN outbox o ON o.tenant=d.tenant AND o.id=d.event_id WHERE d.tenant=$1 AND d.app=$2 AND d.lease_token=$3 ORDER BY o.id",
        t,
        app,
        lease
    )
    .fetch_all(conn)
    .await?)
}

/// The outcome of one leased delivery.
pub struct Outcome<'a> {
    pub state: &'a str,
    pub error: Option<&'a str>,
    pub event_id: i64,
    pub retry_after_seconds: f64,
}

/// Releases a leased delivery with its outcome; a lease another worker took over is left alone.
pub async fn finish(
    a: &veyndo_core::State,
    t: &str,
    app: &str,
    lease: uuid::Uuid,
    o: Outcome<'_>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE app_deliveries SET state=$1,error=$2,lease_until=NULL,lease_token=NULL,available_at=now()+make_interval(secs=>$7) WHERE tenant=$3 AND app=$4 AND event_id=$5 AND lease_token=$6",
        o.state,
        o.error,
        t,
        app,
        o.event_id,
        lease,
        o.retry_after_seconds
    )
    .execute(&a.db)
    .await?;
    Ok(())
}
