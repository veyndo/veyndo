//! Payment provider identity, tenant account configuration and immutable wire context.
use super::remote;
use crate::foundation::App;
use serde_json::Value;
use veyndo_core::Result;
pub(crate) use veyndo_paypal::adapter::{account, base, environment, prepare};

pub(crate) use veyndo_payments::domain::attempt::Attempt;
/// Executes a payment operation at the provider that owns the attempt: the bundled PayPal adapter or the pinned remote service.
pub(crate) async fn dispatch(
    a: &App,
    p: &Attempt,
    op: &str,
    key: &str,
    input: &Value,
) -> Result<Value> {
    if p.provider == "paypal" {
        veyndo_paypal::adapter::execute(a, p, op, key, input).await
    } else {
        remote::execute(a, p, op, key, input).await
    }
}
