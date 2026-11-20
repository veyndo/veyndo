//! Bounded public claim intake/compilation shares current channel admission and source-bound evidence, without per-SKU HTTP round trips.
use super::*;
use veyndo_intelligence::domain::claim_batch::{Compile, Intake};
async fn facts(a: &App, h: &RequestContext, ids: &[String]) -> Result<Vec<Value>> {
    marketing::admit_products(a, h, ids).await?;
    let locale = language_context(a, h).await?.0;
    veyndo_intelligence::store::evidence::claim_facts(a, &tenant(h)?, ids, &locale).await
}
async fn intake(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Intake>,
) -> Result<Json<Value>> {
    let current = facts(&a, &h, &v.product_ids).await?;
    Ok(Json(
        json!({"products":v.product_ids.iter().map(|id|json!({"productId":id,"statements":current.iter().filter(|c|c["productId"]==*id).collect::<Vec<_>>()})).collect::<Vec<_>>() }),
    ))
}
async fn compile(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Compile>,
) -> Result<Json<Value>> {
    v.bounded()?;
    let current = facts(&a, &h, &v.ids()).await?;
    v.exact(&current)?;
    Ok(Json(
        json!({"compiled":true,"products":v.products.iter().map(|p|json!({"productId":p.product_id,"statements":p.claims})).collect::<Vec<_>>(),"scope":"Exact current claims in this language and channel; surrounding prose is not certified"}),
    ))
}
pub(super) fn router() -> Router<App> {
    Router::new()
        .route("/store-api/intelligence/claims", post(intake))
        .route("/store-api/intelligence/claims/compile", post(compile))
}
