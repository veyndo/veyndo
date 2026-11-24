//! Shop lifecycle routes and request availability; the shop component owns both rules.
use crate::{App, RequestContext, Result};
pub(super) use veyndo_shop::http::operator_lifecycle::change;
pub(crate) async fn admit(a: &App, h: &RequestContext, path: &str, method: &str) -> Result<bool> {
    veyndo_shop::service::availability::admit(a, h, path, method).await
}
