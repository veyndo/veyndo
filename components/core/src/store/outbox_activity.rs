//! Recent knowledge activity of a shop as its outbox records it: knowledge and intelligence events and applied merchant changes.
use crate::{Result, State};
use serde_json::Value;

/// The 20 newest knowledge, intelligence and applied-change events of the shop.
pub async fn knowledge(a: &State, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT jsonb_build_object('id',id,'kind',kind,'time',created_at,'sourceId',data->'documentId','hypothesisId',data->'hypothesisId') AS "event!" FROM outbox WHERE tenant=$1 AND (kind LIKE 'knowledge.%' OR kind LIKE 'intelligence.%' OR kind='merchant.change.applied') ORDER BY id DESC LIMIT 20"#,
        t).fetch_all(&a.db).await?)
}
