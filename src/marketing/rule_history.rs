//! Rule comparison lives in the marketing component; these entries keep the server-side signatures.
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) const HISTORY_CAPABILITIES: &[(&str, &str)] = &[(
    "automation.compare_history",
    "Compare a saved rule and draft on frozen order.placed Flow inputs without replaying actions or reading current commerce data",
)];
pub(super) use veyndo_marketing::http::rules::comparison_visible as visible;
pub(super) async fn compare(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    veyndo_marketing::http::rules::compare(State(a.core.clone()), h, Json(v)).await
}
