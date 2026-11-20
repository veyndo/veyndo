//! History transport: bounded, permission-filtered version listings and single versions of the owning shop.
use crate::domain::history_access::{requires_pii, rights};
use crate::store::history::{record, versions};
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};

/// Where a version listing continues.
#[derive(Default, Deserialize)]
pub struct Page {
    /// Only versions older than this one.
    #[serde(default)]
    pub before: Option<i64>,
}

/// Lists the versions of one entity the caller may read.
pub async fn list(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((entity, id)): Path<(String, String)>,
    Query(p): Query<Page>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    if requires_pii(&entity) {
        permit(&h, "customers.pii")?;
    }
    let (read, write) = rights(&entity)?;
    permit(&h, read)?;
    if id.len() > 254 {
        return Err(bad("Invalid history key"));
    }
    let (elements, more) = versions(&a, &t, &entity, &id, p.before).await?;
    Ok(Json(
        json!({"elements":elements,"nextCursor":if more{elements.last().map(|e|e["id"].clone())}else{None},"canRestore":write.is_some_and(|w|permit(&h,w).is_ok()),"entity":entity,"entityId":id,"captureFrom":"034-entity-history","restoresAsNewRevision":true}),
    ))
}

/// Returns one version with its state before and after the change.
pub async fn detail(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((entity, id, version)): Path<(String, String, i64)>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    if requires_pii(&entity) {
        permit(&h, "customers.pii")?;
    }
    permit(&h, rights(&entity)?.0)?;
    Ok(Json(record(&a, &t, &entity, &id, version).await?))
}
