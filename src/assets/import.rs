//! Imported product images: the catalog component stores them; the server supplies the shop's content languages.
use crate::commerce;
use crate::foundation::App;
use crate::foundation::merchant;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
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
