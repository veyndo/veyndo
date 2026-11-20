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
