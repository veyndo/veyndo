//! Per-product channel visibility for the product editor; storage is owned by the catalog component.
use super::*;
pub(crate) async fn product_channels(a: &App, t: &str, id: &str) -> Result<Value> {
    let channels = veyndo_catalog::store::channels::visibility(&a.core, t, id).await?;
    if channels.len() > 100 {
        return Err(bad(
            "Interactive product editor supports at most 100 channels",
        ));
    }
    Ok(json!(channels))
}
