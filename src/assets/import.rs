//! Imported product images: the catalog component stores them; the server supplies the shop's content languages.
use super::*;
pub(crate) async fn image(
    a: &App,
    h: &RequestContext,
    product: &str,
    id: &str,
    metadata: &Value,
    bytes: Vec<u8>,
) -> Result<Value> {
    let (settings, _) = commerce::config(a, &merchant(a, h)?).await?;
    veyndo_catalog::service::asset_access::import_image(
        a,
        h,
        (product, id),
        metadata,
        bytes,
        settings.languages(),
    )
    .await
}
