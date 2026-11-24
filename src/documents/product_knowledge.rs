//! Product knowledge route; the intelligence component owns the facts and graph evidence.
use super::{Knowledge, product_questions};
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn product_knowledge(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        product_questions::product_knowledge(&Knowledge(&a), &h, &id).await?,
    ))
}
