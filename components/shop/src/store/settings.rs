//! The settings of a sales channel read under a share lock, so they cannot change before the transaction commits.
use crate::domain::defaults::decode;
use crate::domain::patch;
use crate::types::settings::Settings;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use veyndo_core::{Error, Result, StatusCode};

/// The shared basis for `default`, otherwise the basis with the channel's overrides applied; `conditions` checks rule conditions in them.
pub async fn scoped_locked(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    channel: &str,
    conditions: &(dyn Fn(&Value) -> Result<()> + Sync),
) -> Result<(Settings, i64)> {
    let row = sqlx::query("SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR SHARE")
        .bind(t)
        .fetch_one(&mut **tx)
        .await?;
    let base = decode(row.get("data"))?;
    let revision: i64 = row.get("revision");
    if channel == "default" {
        return Ok((base, revision));
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2)")
            .bind(t)
            .bind(channel)
            .fetch_one(&mut **tx)
            .await?;
    if !exists {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Sales channel not found".into(),
        ));
    }
    let patch: Option<Value> =
        sqlx::query_scalar("SELECT data FROM commerce_overrides WHERE tenant=$1 AND channel_id=$2")
            .bind(t)
            .bind(channel)
            .fetch_optional(&mut **tx)
            .await?;
    Ok((
        patch::resolve(&base, patch.unwrap_or(json!([])), conditions)?,
        revision,
    ))
}

/// The basis settings of the shop under a share lock on the caller's connection.
pub async fn shared(tx: &mut sqlx::PgConnection, t: &str) -> Result<Settings> {
    let data: Value =
        sqlx::query_scalar("SELECT data FROM commerce_settings WHERE tenant=$1 FOR SHARE")
            .bind(t)
            .fetch_one(tx)
            .await?;
    decode(data)
}

/// The basis settings row and revision of the shop under an update lock on the caller's connection.
pub async fn for_update(tx: &mut sqlx::PgConnection, t: &str) -> Result<(Settings, i64)> {
    let row = sqlx::query("SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
        .bind(t)
        .fetch_one(tx)
        .await?;
    Ok((decode(row.get("data"))?, row.get("revision")))
}

/// Up to 50 shops after `cursor` whose currency rates refresh automatically, with their settings revision.
pub async fn auto_refresh(a: &veyndo_core::State, cursor: &str) -> Result<Vec<(String, i64)>> {
    let rows = sqlx::query("SELECT tenant,revision FROM commerce_settings WHERE data->'currencies'->>'autoRefresh'='true' AND tenant>$1 ORDER BY tenant LIMIT 50")
        .bind(cursor)
        .fetch_all(&a.db)
        .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get("tenant"), r.get("revision")))
        .collect())
}
