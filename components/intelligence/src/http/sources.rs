//! Revision-bound source detail, editing, archive and publication; an edit needs a new public review.
use crate::domain::source_content::admit_guidance;
use crate::service::sources::validate_content;
use crate::store::documents as store;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::view;
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{merchant, permit};

/// One source with its current revision.
pub async fn detail(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "knowledge.read")?;
    Ok(Json(store::detail(&a, &t, &id).await?))
}

/// The JSON fields `edit` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct EditInput {
    kind: Value,
}

/// Edits a source in the shop's languages; guidance sources keep their own admission.
pub async fn edit(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: EditInput = view(&v);
    let t = merchant(&h)?;
    permit(&h, "knowledge.write")?;
    admit_guidance(&h, &input.kind)?;
    let data = validate_content(&a, &t, &v).await?;
    Ok(Json(store::edit(&a, &h, &t, &id, &v, &data).await?))
}

/// Archives, restores, publishes or withdraws a source.
pub async fn lifecycle(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "knowledge.write")?;
    Ok(Json(store::lifecycle(&a, &h, &t, &id, &v).await?))
}
