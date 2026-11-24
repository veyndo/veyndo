//! Admission of a category for the visitor's sales channel; the navigation itself is owned by the catalog component.
use crate::App;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn admit(a: &App, h: &RequestContext, id: &str) -> Result<()> {
    veyndo_catalog::service::navigation::admit(&axum::extract::FromRef::from_ref(a), h, id).await
}
