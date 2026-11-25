//! Stores refreshed reference rates in the shop settings under the expected revision; the refresh never runs inside a checkout.
use crate::service::read_cache::config;
use serde_json::{Value, json};
use veyndo_core::store::history_context::context;
use veyndo_core::{RequestContext, Result, State, conflict};
use veyndo_pricing::service::rates::fetch;

/// Fetches current reference rates and writes them when they changed and the settings still have the expected revision.
pub async fn refresh(a: &State, t: &str, expected: i64, actor: &RequestContext) -> Result<Value> {
    let (s, revision) = config(a, t).await?;
    if revision != expected {
        return Err(conflict("Settings changed; reload first"));
    }
    let next = fetch(&s.currencies).await?;
    if next == s.currencies {
        return Ok(json!({"currencies":next,"revision":revision}));
    }
    let mut tx = a.db.begin().await?;
    context(&mut tx, actor, "currency-rate-refresh").await?;
    sqlx::query("UPDATE commerce_settings SET data=jsonb_set(data,'{currencies}',$1),revision=revision+1 WHERE tenant=$2 AND revision=$3 RETURNING revision").bind(json!(next)).bind(t).bind(expected).fetch_optional(&mut *tx).await?.ok_or(conflict("Settings changed during rate refresh"))?;
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        t,
        "commerce.configured",
        &json!({"revision":expected+1,"reason":"currency_rates"}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"currencies":next,"revision":expected+1}))
}
