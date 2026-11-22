//! Reference rate refresh lives in the shop component; this entry keeps the server-side signature.
use super::*;
pub(super) async fn refresh(
    a: &App,
    t: &str,
    expected: i64,
    actor: &RequestContext,
) -> Result<Value> {
    veyndo_shop::service::currency_rates::refresh(&a.core, t, expected, actor).await
}
