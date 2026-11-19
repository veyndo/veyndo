//! Priority-based destination rules; current tax law is merchant configuration, not bundled tax advice.
use super::*;
pub(crate) fn destination_rate(t: &TaxConfig, c: &CheckoutSelection, today: &str) -> Result<f64> {
    let address = c.address.as_ref();
    veyndo_pricing::service::tax::destination_rate(
        t,
        &veyndo_pricing::types::tax::Destination {
            country: &c.country,
            state: address.map(|a| a.country_state_id.as_str()).unwrap_or(""),
            postal_code: address.map(|a| a.postal_code.as_str()).unwrap_or(""),
        },
        today,
    )
}
