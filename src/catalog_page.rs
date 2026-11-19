//! Storefront catalogue pages; criteria, queries and translation are owned by the catalog component.
use crate::{App, Result};
pub(crate) use veyndo_catalog::types::catalog_page::{CatalogCriteria, CatalogPage};

pub(crate) async fn product_page(
    a: &App,
    t: &str,
    chain: &[String],
    criteria: &CatalogCriteria,
) -> Result<CatalogPage> {
    veyndo_catalog::service::catalog_reads::page(
        &axum::extract::FromRef::from_ref(a),
        t,
        chain,
        criteria,
    )
    .await
}
