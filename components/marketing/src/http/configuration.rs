//! Revision-bound deletion and the dependency read of marketing configuration share HTTP/MCP authorization and tenant locks.
use crate::domain::configuration_kinds::configuration_right;
use crate::store::configuration_lifecycle::{lock, revision};
use crate::store::configuration_writes as rows;
use crate::store::dependencies::inspect;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, bad, conflict, store::history_context};
use veyndo_identity::{merchant, permit};

/// What still uses a definition, with its current revision.
pub async fn dependencies(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path((kind, id)): Path<(String, String)>,
) -> Result<Json<Value>> {
    permit(&h, configuration_right(&kind, false)?)?;
    let t = merchant(&h)?;
    let mut conn = a.db.begin().await?;
    let rev = revision(&mut conn, &t, &kind, &id, false).await?;
    let mut v = inspect(&mut conn, &t, &kind, &id).await?;
    v["revision"] = json!(rev);
    Ok(Json(v))
}
/// The JSON fields `delete` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DeleteInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Deletes an unused definition at the expected revision.
pub async fn delete(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path((kind, id)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: DeleteInput = view(&v);
    permit(&h, configuration_right(&kind, true)?)?;
    let t = merchant(&h)?;
    let expected = input.revision.ok_or(bad("Revision required"))?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, &h, "merchant").await?;
    lock(&mut tx, &t).await?;
    let rev = revision(&mut tx, &t, &kind, &id, true).await?;
    if rev != expected {
        return Err(conflict("Configuration revision changed"));
    }
    let deps = inspect(&mut tx, &t, &kind, &id).await?;
    if view::<Blocked>(&deps).blocked == true {
        return Err(conflict(
            "Configuration is in use; remove references or deactivate it first",
        ));
    }
    rows::delete(&mut tx, &t, &kind, &id, rev).await?;
    tx.commit().await?;
    Ok(Json(json!({"deleted":true,"id":id,"revision":rev})))
}

/// Whether something still uses a definition.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Blocked {
    blocked: Value,
}
