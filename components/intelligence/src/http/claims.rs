//! Public claim intake and compilation: bounded batches under the current channel admission, without one request per product.
use crate::domain::claim_batch::{Compile, Intake};
use crate::service::claims::facts;
use axum::Json;
use axum::extract::State as Shared;
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State};

/// The current statements of each requested product.
pub async fn intake(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Intake>,
) -> Result<Json<Value>> {
    let current = facts(&a, &h, &v.product_ids).await?;
    Ok(Json(
        json!({"products":v.product_ids.iter().map(|id|json!({"productId":id,"statements":current.iter().filter(|c|c["productId"]==*id).collect::<Vec<_>>()})).collect::<Vec<_>>() }),
    ))
}

/// Certifies that a batch quotes exactly the current claims.
pub async fn compile(
    Shared(a): Shared<State>,
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
