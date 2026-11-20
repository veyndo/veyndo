//! Bounded metadata-only app telemetry: who called which action with what outcome, never a payload.
use serde_json::Value;
use veyndo_core::shop_scope::tenant;
use veyndo_core::{RequestContext, State, StatusCode, hash};

/// Records one action call and keeps the latest 1000 per app.
pub async fn record(
    a: &State,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
    status: StatusCode,
    elapsed: u128,
) {
    let Ok(t) = tenant(h) else {
        return;
    };
    let actor = h.principal.user.as_deref().unwrap_or("anonymous");
    let inserted=sqlx::query("INSERT INTO app_action_log(tenant,app,action,actor,status,duration_ms,input_digest) SELECT $1,$2,$3,$4,$5,$6,$7 WHERE EXISTS(SELECT 1 FROM app_packages WHERE tenant=$1 AND id=$2)").bind(&t).bind(id).bind(name).bind(actor).bind(status.as_u16() as i32).bind(elapsed.min(i64::MAX as u128) as i64).bind(hash(&v.to_string())).execute(&a.db).await;
    if inserted.is_err() {
        eprintln!("App action metadata could not be recorded");
    }
    // Keyset retention is bounded and never stores payloads; keep the latest 1000 calls per app.
    let _=sqlx::query("DELETE FROM app_action_log WHERE tenant=$1 AND app=$2 AND id<(SELECT id FROM app_action_log WHERE tenant=$1 AND app=$2 ORDER BY id DESC OFFSET 999 LIMIT 1)").bind(t).bind(id).execute(&a.db).await;
}
