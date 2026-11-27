//! Stores refreshed reference rates in the shop settings under the expected revision; the refresh never runs inside a checkout.
use crate::service::read_cache::config;
use crate::store::settings::replace_currencies_at;
use crate::types::events::RatesRefreshed;
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
    replace_currencies_at(&mut tx, t, &json!(next), expected)
        .await?
        .ok_or(conflict("Settings changed during rate refresh"))?;
    let refreshed = RatesRefreshed {
        revision: expected + 1,
        reason: "currency_rates",
    };
    veyndo_core::events::emit(&mut *tx, t, &refreshed).await?;
    tx.commit().await?;
    Ok(json!({"currencies":next,"revision":expected+1}))
}
