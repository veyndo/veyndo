//! Payment provider identity, tenant account configuration and immutable wire context.
use super::remote;
#[path = "../../extensions/apps/paypal/src/adapter.rs"]
mod legacy;
use crate::App;
pub(crate) use legacy::{account, base, environment, prepare};
use serde_json::Value;
use veyndo_core::Result;

pub(crate) use veyndo_payments::domain::amounts::{format_amount, parse_amount};
pub(crate) use veyndo_payments::domain::attempt::Attempt;
/// Each adapter implements this command boundary; the core owns ledger transitions.
pub(crate) trait PaymentProvider {
    async fn execute(
        &self,
        a: &App,
        p: &Attempt,
        op: &str,
        key: &str,
        input: &Value,
    ) -> Result<Value>;
}
pub(crate) async fn dispatch(
    a: &App,
    p: &Attempt,
    op: &str,
    key: &str,
    input: &Value,
) -> Result<Value> {
    if p.provider == "paypal" {
        legacy::execute(a, p, op, key, input).await
    } else {
        remote::execute(a, p, op, key, input).await
    }
}
