//! Consent-bound private cart preference graph: entry points, consent and channel admission; model and store live in the intelligence component.
use crate::*;
use veyndo_intelligence::domain::preference_graph::Graph;
use veyndo_intelligence::store::preferences as store;
pub(crate) use veyndo_intelligence::store::preferences::{AdviceContext, forget_many};
async fn read(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    legal::require(&a, &c, "personalization").await?;
    Ok(Json(store::read(&a, &c.tenant, &c.id).await?))
}
async fn save(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    legal::require(&a, &c, "personalization").await?;
    let g: Graph =
        serde_json::from_value(v["graph"].clone()).map_err(|_| bad("Invalid preference graph"))?;
    g.validate()?;
    let revision = v["revision"]
        .as_i64()
        .filter(|r| *r >= 0)
        .ok_or(bad("Preference revision required"))?;
    for n in &g.nodes {
        if let Some(id) = &n.product_id {
            marketing::admit_product(&a, &h, id).await?;
        }
    }
    let mut tx = a.db.begin().await?;
    legal::require_locked(&mut tx, &c, "personalization").await?;
    store::write(
        &mut tx,
        &c.tenant,
        &c.id,
        &g,
        v["useForAdvice"] == true,
        revision,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"revision":revision+1,"saved":true})))
}
async fn delete(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let mut tx = a.db.begin().await?;
    forget(&mut tx, &c).await?;
    tx.commit().await?;
    Ok(Json(json!({"forgotten":true})))
}
pub(crate) async fn forget(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
) -> Result<()> {
    forget_many(tx, &c.tenant, std::slice::from_ref(&c.id)).await
}
pub(crate) async fn capture(a: &App, h: &RequestContext) -> Result<AdviceContext> {
    let Ok(c) = load_cart(a, h).await else {
        return Ok(AdviceContext::empty());
    };
    if legal::require(a, &c, "personalization").await.is_err() {
        return Ok(AdviceContext::empty());
    }
    store::advice(a, &c.tenant, &c.id).await
}
pub(crate) fn router() -> Router<App> {
    Router::new().route(
        "/store-api/intelligence/preferences",
        get(read).put(save).delete(delete),
    )
}
