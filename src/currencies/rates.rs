//! Reference rate refresh lives in the shop component; this entry keeps the server-side signature.
use crate::foundation::App;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(super) async fn refresh(
    a: &App,
    t: &str,
    expected: i64,
    actor: &RequestContext,
) -> Result<Value> {
    veyndo_shop::service::currency_rates::refresh(&a.core, t, expected, actor).await
}
