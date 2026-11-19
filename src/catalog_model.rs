//! The shared Commerce product contract and its row hydration, owned by the catalog component.
use crate::{App, Result};
pub(crate) use veyndo::catalog::Product;
pub(crate) use veyndo_catalog::store::products::product;
pub(crate) async fn prototype_products(a: &App, t: &str) -> Result<Vec<Product>> {
    veyndo_catalog::store::products::prototype(&a.core, t).await
}
