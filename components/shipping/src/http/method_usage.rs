//! Where a delivery or payment method is in use.
use crate::ports::MethodReferences;
use crate::service::method_usage;
use crate::store::method_usage::Usage;
use axum::Json;
use axum::extract::{Path, State};
use veyndo_core::{RequestContext, Result};

pub async fn dependencies(
    State(a): State<veyndo_core::State>,
    references: &dyn MethodReferences,
    h: RequestContext,
    Path((area, id)): Path<(String, String)>,
) -> Result<Json<Usage>> {
    Ok(Json(
        method_usage::dependencies(&a, references, &h, &area, &id).await?,
    ))
}
