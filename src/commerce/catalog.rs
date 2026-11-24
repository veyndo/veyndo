//! SKU loading with parent translation fallback; owned by the catalog component.
use crate::{App, Item, Product, Result};
use axum::extract::FromRef;
use veyndo_catalog::service::catalog_reads;

pub(crate) async fn cart_products(
    a: &App,
    t: &str,
    chain: &[String],
    items: &[Item],
) -> Result<Vec<Product>> {
    let ids = items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    catalog_reads::for_cart(&FromRef::from_ref(a), t, chain, &ids).await
}
