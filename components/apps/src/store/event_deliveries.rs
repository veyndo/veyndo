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
