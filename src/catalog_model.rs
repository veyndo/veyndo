//! The shared Commerce product contract and its row hydration, owned by the catalog component.
use crate::foundation::App;
pub(crate) use veyndo::catalog::Product;
use veyndo_core::Result;
pub(crate) async fn prototype_products(a: &App, t: &str) -> Result<Vec<Product>> {
    veyndo_catalog::store::products::prototype(&a.core, t).await
}
