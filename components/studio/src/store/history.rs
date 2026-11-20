//! Reads the recorded versions of one entity of a shop.
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Error, Result, State, StatusCode};

/// The page size of a version listing.
const PAGE: usize = 30;

/// Up to thirty versions older than `before`, newest first, and whether older ones remain.
pub async fn versions(
    a: &State,
    t: &str,
    entity: &str,
    id: &str,
    before: Option<i64>,
) -> Result<(Vec<Value>, bool)> {
    let rows=sqlx::query("SELECT eh.id,actor,coalesce(u.name,actor) AS actor_label,source,reason,eh.created_at::text AS time,before_state IS NOT NULL AS restorable,coalesce(after_state->'revision',after_state->'record'->'revision') AS revision FROM entity_history eh LEFT JOIN merchant_users u ON u.id=eh.actor WHERE eh.tenant=$1 AND entity=$2 AND entity_id=$3 AND eh.id<$4 AND before_state IS DISTINCT FROM after_state ORDER BY eh.id DESC LIMIT 31").bind(t).bind(entity).bind(id).bind(before.unwrap_or(i64::MAX)).fetch_all(&a.db).await?;
    let elements=rows.iter().take(PAGE).map(|r|json!({"id":r.get::<i64,_>("id"),"actor":r.get::<Option<String>,_>("actor"),"actorLabel":r.get::<Option<String>,_>("actor_label"),"source":r.get::<String,_>("source"),"reason":r.get::<Option<String>,_>("reason"),"createdAt":r.get::<String,_>("time"),"revision":r.get::<Option<Value>,_>("revision"),"hasPrevious":r.get::<bool,_>("restorable")})).collect::<Vec<_>>();
    Ok((elements, rows.len() > PAGE))
}

/// One version with the state before and after the change.
pub async fn record(a: &State, t: &str, entity: &str, id: &str, version: i64) -> Result<Value> {
    sqlx::query_scalar("SELECT jsonb_build_object('id',id,'before',before_state,'after',after_state) FROM entity_history WHERE tenant=$1 AND entity=$2 AND entity_id=$3 AND id=$4")
        .bind(t).bind(entity).bind(id).bind(version).fetch_optional(&a.db).await?.ok_or(Error(StatusCode::NOT_FOUND,"History version unavailable".into()))
}
