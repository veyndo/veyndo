//! The developer builds of a shop: the latest app versions a generation must not repeat.
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Result, State};

/// The 30 latest builds as `{app, version}`.
pub async fn recent(a: &State, t: &str) -> Result<Value> {
    let rows = sqlx::query(
        "SELECT app,version FROM developer_builds WHERE tenant=$1 ORDER BY created_at DESC LIMIT 30",
    )
    .bind(t)
    .fetch_all(&a.db)
    .await?;
    Ok(json!(
        rows.iter()
            .map(|r| json!({"app":r.get::<String,_>("app"),"version":r.get::<String,_>("version")}))
            .collect::<Vec<_>>()
    ))
}
