//! Entry points of product file operations for capabilities and app upload surfaces; rules and storage are owned by the catalog component.
use crate::App;
use axum::Json;
use axum::extract::FromRef;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn list(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    veyndo_catalog::http::assets::list(State(a.core.clone()), h, Path(id)).await
}
pub(crate) async fn persist(
    a: &App,
    h: &RequestContext,
    product: &str,
    upload: super::ingestion::Upload,
) -> Result<Value> {
    veyndo_catalog::service::assets::persist(&FromRef::from_ref(a), h, product, upload).await
}
pub(crate) async fn publish(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    veyndo_catalog::http::assets::publish(State(a.core.clone()), h, Path(id), Json(v)).await
}
