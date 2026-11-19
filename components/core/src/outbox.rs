//! The transactional outbox consumer: each subscriber lane acknowledges an event and commits its effects in one transaction.
use crate::{Result, State, bad};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::{future::Future, pin::Pin};
use tracing::Instrument;
pub mod retention;

pub type Projection<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// What the subscribers outside this crate do with one event; the server wires the components in.
pub trait Projector: Sync {
    fn project<'a>(
        &'a self,
        tx: &'a mut Transaction<'static, Postgres>,
        subscriber: &'a str,
        tenant: &'a str,
        event: i64,
        kind: &'a str,
        data: &'a Value,
    ) -> Projection<'a>;
}

pub const SUBSCRIBERS: [&str; 4] = ["cognition", "apps", "flows", "audit"];
/// Each lane owns its schedule; exact event acknowledgement and effects commit atomically.
pub async fn consume(a: &State, subscriber: &str, projector: &dyn Projector) -> Result<bool> {
    if !SUBSCRIBERS.contains(&subscriber) {
        return Err(bad("Unknown outbox subscriber"));
    }
    let bit = 1_i16 << SUBSCRIBERS.iter().position(|s| *s == subscriber).unwrap();
    // Closed server constants let PostgreSQL use the matching partial index even
    // with a generic prepared plan. No caller text enters either SQL statement.
    let scheduling =
        include_str!("outbox/next_tenant.sql").replace("SUBSCRIBER_BIT", &bit.to_string());
    let mut tx = a.db.begin().await?;
    let next = sqlx::query(sqlx::AssertSqlSafe(scheduling.as_str()))
        .bind(subscriber)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(next) = next else {
        tx.commit().await?;
        return Ok(false);
    };
    let t: String = next.get("tenant");
    let selection = format!(
        "SELECT e.* FROM outbox e LEFT JOIN outbox_subscriber_failures f ON f.tenant=e.tenant AND f.event_id=e.id AND f.subscriber=$2 WHERE e.tenant=$1 AND (e.subscriber_mask & {bit}::smallint)=0 AND NOT e.dispatch_requires_retry AND e.dispatch_not_before<=now() AND e.delivered_at IS NULL AND e.payload_retired_at IS NULL AND f.dead_letter_at IS NULL AND coalesce(f.available_at,'-infinity'::timestamptz)<=now() ORDER BY e.id LIMIT 50"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(selection.as_str()))
        .bind(&t)
        .bind(subscriber)
        .fetch_all(&mut *tx)
        .await?;
    sqlx::query("UPDATE outbox_checkpoints SET served_order=nextval('public.outbox_dispatch_sequence') WHERE tenant=$1 AND subscriber=$2")
        .bind(&t).bind(subscriber).execute(&mut *tx).await?;
    let worked = !rows.is_empty();
    for r in rows {
        let id = r.get::<i64, _>("id");
        let kind = r.get::<String, _>("kind");
        let data = r.get::<Value, _>("data");
        sqlx::query("SAVEPOINT project_event")
            .execute(&mut *tx)
            .await?;
        let result:Result<()> = async {
            match subscriber {
                "audit"=>{sqlx::query("INSERT INTO projections(event_id,tenant,kind,data) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(id).bind(&t).bind(&kind).bind(&data).execute(&mut *tx).await?;},
                _=>projector.project(&mut tx,subscriber,&t,id,&kind,&data).await?,
            }
            sqlx::query("UPDATE outbox SET subscriber_mask=subscriber_mask|$3::smallint,attempts=CASE WHEN (subscriber_mask|$3::smallint)=15 THEN 0 ELSE attempts END,delivered_at=CASE WHEN (subscriber_mask|$3::smallint)=15 THEN now() ELSE delivered_at END,dead_letter_at=CASE WHEN (subscriber_mask|$3::smallint)=15 THEN NULL ELSE dead_letter_at END,error_code=CASE WHEN (subscriber_mask|$3::smallint)=15 THEN NULL ELSE error_code END WHERE tenant=$1 AND id=$2")
                .bind(&t).bind(id).bind(bit).execute(&mut *tx).await?;
            sqlx::query("UPDATE outbox_checkpoints SET last_event_id=greatest(last_event_id,$3) WHERE tenant=$1 AND subscriber=$2")
                .bind(&t).bind(subscriber).bind(id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM outbox_subscriber_failures WHERE tenant=$1 AND event_id=$2 AND subscriber=$3")
                .bind(&t).bind(id).bind(subscriber).execute(&mut *tx).await?;
            Ok(())
        }.instrument(tracing::info_span!("outbox_event",trace_id=%r.get::<String,_>("trace_id"),event_id=id,subscriber=subscriber)).await;
        if let Err(error) = result {
            sqlx::query("ROLLBACK TO SAVEPOINT project_event")
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO outbox_subscriber_failures(tenant,event_id,subscriber,available_at) VALUES($1,$2,$3,now()+interval '2 seconds') ON CONFLICT(tenant,event_id,subscriber) DO UPDATE SET attempts=outbox_subscriber_failures.attempts+1,available_at=now()+least(300,power(2,least(outbox_subscriber_failures.attempts+1,8))::int)*interval '1 second',dead_letter_at=CASE WHEN outbox_subscriber_failures.attempts+1>=8 THEN now() ELSE NULL END")
                .bind(&t).bind(id).bind(subscriber).execute(&mut *tx).await?;
            // Preserve another subscriber's quarantine until explicit retry or complete acknowledgement.
            sqlx::query("UPDATE outbox e SET attempts=greatest(e.attempts,f.attempts),available_at=f.available_at,dead_letter_at=coalesce(e.dead_letter_at,f.dead_letter_at),error_code='projection_failed' FROM outbox_subscriber_failures f WHERE e.id=$1 AND e.tenant=$2 AND f.tenant=e.tenant AND f.event_id=e.id AND f.subscriber=$3")
                .bind(id).bind(&t).bind(subscriber).execute(&mut *tx).await?;
            eprintln!(
                "outbox_projection_failed tenant={t} event_id={id} subscriber={subscriber} kind={kind} status={}",
                error.0.as_u16()
            );
            sqlx::query("RELEASE SAVEPOINT project_event")
                .execute(&mut *tx)
                .await?;
            continue;
        }
        sqlx::query("RELEASE SAVEPOINT project_event")
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(worked)
}

/// Records an event in the same transaction as the change it describes.
pub async fn publish(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
    kind: &str,
    data: serde_json::Value,
) -> Result<()> {
    sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,$2,$3)")
        .bind(tenant)
        .bind(kind)
        .bind(data)
        .execute(tx)
        .await?;
    Ok(())
}
