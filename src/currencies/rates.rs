//! Stores refreshed reference rates in the shop settings; the refresh runs outside checkout.
use super::*;
use veyndo_pricing::service::rates::fetch;
pub(super) async fn refresh(
    a: &App,
    t: &str,
    expected: i64,
    actor: &RequestContext,
) -> Result<Value> {
    let (s, revision) = commerce::config(a, t).await?;
    if revision != expected {
        return Err(conflict("Settings changed; reload first"));
    }
    let next = fetch(&s.currencies).await?;
    if next == s.currencies {
        return Ok(json!({"currencies":next,"revision":revision}));
    }
    let mut tx = a.db.begin().await?;
    history::context(&mut tx, actor, "currency-rate-refresh").await?;
    sqlx::query("UPDATE commerce_settings SET data=jsonb_set(data,'{currencies}',$1),revision=revision+1 WHERE tenant=$2 AND revision=$3 RETURNING revision").bind(json!(next)).bind(t).bind(expected).fetch_optional(&mut *tx).await?.ok_or(conflict("Settings changed during rate refresh"))?;
    sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,'commerce.configured',$2)")
        .bind(t)
        .bind(json!({"revision":expected+1,"reason":"currency_rates"}))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(json!({"currencies":next,"revision":expected+1}))
}
