//! Durable outbox and audit projection worker.
use crate::*;
mod control;
mod retention;
pub(crate) use control::retry as retry_outbox;

pub(crate) async fn runtime(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    let r=sqlx::query("SELECT (SELECT count(*) FROM outbox WHERE tenant=$1 AND delivered_at IS NULL AND dead_letter_at IS NULL) AS pending,(SELECT count(*) FROM outbox WHERE tenant=$1 AND dead_letter_at IS NOT NULL) AS quarantined,(SELECT count(*) FROM outbox WHERE tenant=$1 AND delivered_at IS NULL AND attempts>0 AND dead_letter_at IS NULL) AS retrying,(SELECT count(*) FROM projections WHERE tenant=$1) AS consumed").bind(t).fetch_one(&a.db).await?;
    Ok(Json(
        json!({"outboxPending":r.get::<i64,_>("pending"),"outboxQuarantined":r.get::<i64,_>("quarantined"),"outboxRetrying":r.get::<i64,_>("retrying"),"eventsConsumed":r.get::<i64,_>("consumed"),"consumer":"independent cognition/apps/flows/audit cursors; no external messages in projection",
            // Instance-wide diagnostic counters are visible only to the existing instance credential.
            "performance":if header(&h,"x-rac-user")==Some("bootstrap") {Some(json!({"reads":a.reads.snapshot(),"admission":a.admission.snapshot(),"pool":{"size":a.db.size(),"idle":a.db.num_idle()}}))}else{None}}),
    ))
}
pub(crate) use veyndo_core::outbox::SUBSCRIBERS;

/// The subscribers that live outside the core crate.
struct Components;
impl veyndo_core::outbox::Projector for Components {
    fn project<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'static, sqlx::Postgres>,
        subscriber: &'a str,
        tenant: &'a str,
        event: i64,
        kind: &'a str,
        data: &'a Value,
    ) -> veyndo_core::outbox::Projection<'a> {
        Box::pin(async move {
            match subscriber {
                "cognition" => cognition::project(tx, tenant, event, kind, data).await,
                "apps" => apps::project_events(tx, tenant, event, kind).await,
                "flows" => marketing::project_flows(tx, tenant, event, kind, data).await,
                _ => unreachable!(),
            }
        })
    }
}

pub(crate) async fn consume_subscriber(a: &App, subscriber: &str) -> Result<bool> {
    let worked = veyndo_core::outbox::consume(a, subscriber, &Components).await?;
    retention::once(a).await?;
    Ok(worked)
}
