//! Admission of a category for the visitor's sales channel; the navigation itself is owned by the catalog component.
use crate::{App, RequestContext, Result};
pub(crate) async fn admit(a: &App, h: &RequestContext, id: &str) -> Result<()> {
    veyndo_catalog::service::navigation::admit(&axum::extract::FromRef::from_ref(a), h, id).await
}
