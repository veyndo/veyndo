//! Entry points of product file operations for capabilities and app upload surfaces; rules and storage are owned by the catalog component.
use super::*;
use axum::extract::FromRef;
pub(super) use veyndo_catalog::domain::asset_file::validate_bytes;
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
