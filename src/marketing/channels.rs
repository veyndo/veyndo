//! Sales channel admission lives in the marketing component; these entries keep the server-side signatures.
use crate::foundation::App;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_marketing::service::channels as owner;
pub(crate) use veyndo_marketing::types::channel::Channel;
pub(crate) async fn channel(
    a: &App,
    h: &RequestContext,
    id: &str,
    locale: &str,
) -> Result<Option<Channel>> {
    owner::channel(&a.core, h, id, locale).await
}
pub(crate) use veyndo_core::shop_scope::channel as channel_id;
pub(crate) async fn admit_product(a: &App, h: &RequestContext, id: &str) -> Result<()> {
    owner::admit_product(&a.core, h, id).await
}
pub(crate) async fn admit_products(a: &App, h: &RequestContext, ids: &[String]) -> Result<()> {
    owner::admit_products(&a.core, h, ids).await
}
pub(crate) async fn catalog_scope(a: &App, h: &RequestContext) -> Result<Option<Vec<String>>> {
    owner::catalog_scope(&a.core, h).await
}
