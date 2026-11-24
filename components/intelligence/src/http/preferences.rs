//! Consent-bound private cart preference graph: storefront reads and writes with consent and channel admission.
use crate::domain::preference_graph::Graph;
use crate::service::consent::{require, require_locked};
use crate::store::preferences::{self as store, AdviceContext, forget_many};
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_checkout::store::cart_load::load as load_cart;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_marketing::service::channels::admit_product;
use veyndo_shop::types::cart::StoredCart;

/// The cart's private preference graph.
pub async fn read(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    require(&a, &c, "personalization").await?;
    Ok(Json(store::read(&a, &c.tenant, &c.id).await?))
}
/// The JSON fields `save` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveInput {
    graph: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    use_for_advice: Option<bool>,
}

/// Saves the cart's preference graph at its revision.
pub async fn save(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: SaveInput = view(&v);
    let c = load_cart(&a, &h).await?;
    require(&a, &c, "personalization").await?;
    let g: Graph =
        serde_json::from_value(input.graph.clone()).map_err(|_| bad("Invalid preference graph"))?;
    g.validate()?;
    let revision = input
        .revision
        .filter(|r| *r >= 0)
        .ok_or(bad("Preference revision required"))?;
    for n in &g.nodes {
        if let Some(id) = &n.product_id {
            admit_product(&a, &h, id).await?;
        }
    }
    let mut tx = a.db.begin().await?;
    require_locked(&mut tx, &c, "personalization").await?;
    store::write(
        &mut tx,
        &c.tenant,
        &c.id,
        &g,
        input.use_for_advice == Some(true),
        revision,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"revision":revision+1,"saved":true})))
}
/// Forgets the cart's preference graph.
pub async fn delete(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let mut tx = a.db.begin().await?;
    forget(&mut tx, &c).await?;
    tx.commit().await?;
    Ok(Json(json!({"forgotten":true})))
}
/// Forgets the preference graph of one cart inside the caller's transaction.
pub async fn forget(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, c: &StoredCart) -> Result<()> {
    forget_many(tx, &c.tenant, std::slice::from_ref(&c.id)).await
}
/// The consent-bound advice context of the request's cart; empty without consent.
pub async fn capture(a: &State, h: &RequestContext) -> Result<AdviceContext> {
    let Ok(c) = load_cart(a, h).await else {
        return Ok(AdviceContext::empty());
    };
    if require(a, &c, "personalization").await.is_err() {
        return Ok(AdviceContext::empty());
    }
    store::advice(a, &c.tenant, &c.id).await
}
