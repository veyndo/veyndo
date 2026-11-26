//! Product question route; the intelligence component owns the answer, its citations and the re-admission.
use super::{Knowledge, product_questions};
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn question(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        product_questions::question(&Knowledge(&a), &h, &id, &v).await?,
    ))
}
