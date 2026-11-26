//! The source-bound claim lifecycle and public claim batches: rights and shop languages here, the ledger in `store::evidence`.
use crate::store::evidence as store;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{merchant, permit};
use veyndo_marketing::service::channels::admit_products;
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::config;

/// Proposes a claim in its own transaction.
pub async fn propose(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "knowledge.write")?;
    let mut tx = a.db.begin().await?;
    let result = propose_in(a, &mut tx, &t, h, v).await?;
    tx.commit().await?;
    Ok(result)
}

/// Proposes a claim inside the transaction of the caller, checked against the shop languages.
pub async fn propose_in(
    a: &State,
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    h: &RequestContext,
    v: &Value,
) -> Result<Value> {
    let (settings, _) = config(a, t).await?;
    store::propose_in(
        tx,
        t,
        header(h, "x-rac-user").unwrap_or("integration"),
        v,
        &settings.main_locale,
        &settings.locales,
    )
    .await
}

/// Records the merchant's decision on a claim.
pub async fn decide(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "knowledge.write")?;
    store::decide(a, &t, header(h, "x-rac-user"), v).await
}

/// The current claims of products the channel shows, in the language of the request.
pub async fn facts(a: &State, h: &RequestContext, ids: &[String]) -> Result<Vec<Value>> {
    admit_products(a, h, ids).await?;
    let locale = language_context(a, h).await?.0;
    store::claim_facts(a, &tenant(h)?, ids, &locale).await
}
