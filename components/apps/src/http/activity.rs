//! Bounded metadata-only app telemetry, storage usage and explicitly approved cursor-based event replay.
use crate::domain::event_projection;
use crate::store::packages::package;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::parent;
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::merchant;

/// The latest calls, deliveries and storage use of an app; metadata only.
pub async fn activity(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    package(&a, &t, &id, false).await?;
    let calls:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'action',action,'actor',actor,'status',status,'durationMs',duration_ms,'at',created_at) FROM app_action_log WHERE tenant=$1 AND app=$2 ORDER BY id DESC LIMIT 50").bind(&t).bind(&id).fetch_all(&a.db).await?;
    let deliveries:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('eventId',d.event_id,'kind',o.kind,'state',d.state,'attempts',d.attempts,'error',d.error,'lastAttemptAt',d.last_attempt_at) FROM app_deliveries d JOIN outbox o ON o.tenant=d.tenant AND o.id=d.event_id WHERE d.tenant=$1 AND d.app=$2 ORDER BY d.event_id DESC LIMIT 50").bind(&t).bind(&id).fetch_all(&a.db).await?;
    let usage:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('rows',rows,'bytes',bytes,'rowLimit',100000,'byteLimit',67108864) FROM app_storage_usage WHERE tenant=$1 AND app=$2").bind(t).bind(id).fetch_optional(&a.db).await?;
    Ok(Json(
        json!({"calls":calls,"deliveries":deliveries,"storage":usage,"delivery":"at-least-once","retentionCalls":1000}),
    ))
}
/// The JSON fields `replay` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReplayInput {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    after: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    limit: Option<i64>,
}

/// Queues eligible outbox events after a cursor again; the caller approves repeated external effects.
pub async fn replay(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: ReplayInput = view(&v);
    let t = merchant(&h)?;
    if input.approve != Some(true) {
        return Err(bad(
            "Approve event replay; external effects may occur again",
        ));
    }
    if parent(&a, &t).await?.is_some() {
        return Err(bad("Private sandbox events cannot be delivered externally"));
    }
    let m = package(&a, &t, &id, true).await?;
    let after = input
        .after
        .filter(|n| *n >= 0)
        .ok_or(bad("Nonnegative event cursor required"))?;
    let limit = if v.get("limit").is_none() {
        50
    } else {
        input
            .limit
            .filter(|n| (1..=100).contains(n))
            .ok_or(bad("Replay limit must be 1..100"))?
    };
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,843))")
        .bind(format!("{t}:{id}"))
        .execute(&mut *tx)
        .await?;
    let rows = sqlx::query(
        "SELECT id,kind,data FROM outbox WHERE tenant=$1 AND id>$2 ORDER BY id LIMIT $3",
    )
    .bind(&t)
    .bind(after)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await?;
    let mut queued = 0;
    for r in &rows {
        let kind: String = r.get("kind");
        let data: Value = r.get("data");
        if event_projection::eligible(&m, &kind, &data) {
            queued+=sqlx::query("INSERT INTO app_deliveries(tenant,app,event_id) VALUES($1,$2,$3) ON CONFLICT(tenant,app,event_id) DO UPDATE SET state='queued',attempts=0,error=NULL,available_at=now(),lease_until=NULL,lease_token=NULL WHERE app_deliveries.state IN ('failed','delivered')").bind(&t).bind(&id).bind(r.get::<i64,_>("id")).execute(&mut *tx).await?.rows_affected();
        }
    }
    veyndo_core::outbox::publish_on(
        &mut *tx,
        &t,
        "app.replay_requested",
        &json!({"app":id,"after":after,"queued":queued,"actor":h.principal.user}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"queued":queued,"nextCursor":rows.last().map(|r|r.get::<i64,_>("id")).unwrap_or(after),"hasMore":rows.len()==limit as usize}),
    ))
}
