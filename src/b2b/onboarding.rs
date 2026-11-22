//! Atomic company setup lives in the B2B component; this entry keeps the server-side signature.
use super::*;
pub(super) async fn create(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_b2b::service::onboarding::create(&a.core, h, v).await
}
