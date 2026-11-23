//! The company basis lives in the orders component; these entries keep the server-side signatures.
use super::*;
pub(crate) use veyndo_orders::store::company::{lock, public_source};
pub(super) async fn read(
    a: &App,
    h: &RequestContext,
    channel: Option<&str>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_orders::http::company::read(&a.core, h, channel).await?,
    ))
}
pub(crate) async fn save(
    a: &App,
    h: &RequestContext,
    v: Value,
    channel: Option<&str>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_orders::http::company::save(&a.core, h, v, channel).await?,
    ))
}
