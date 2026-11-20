//! Requisition import is owned by the B2B component; merchant admission and shop settings are resolved here.
use super::*;
pub(super) async fn import(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "b2b.manage")?;
    let (settings, _) = commerce::config(a, &t).await?;
    veyndo_b2b::service::requisition::import(a, h, &t, &settings, v).await
}
