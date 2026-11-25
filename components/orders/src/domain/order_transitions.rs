//! Revision-free order transition rules: manual payment marks, delivery dispatch and the cancellation preconditions.
use crate::domain::order_machine::OrderMachine;
use crate::domain::order_patches::{Stated, TrackingCode, TrackingUrl};
use crate::domain::order_workflow::external;
use crate::types::order_document::DeliveryControl;
use serde::Deserialize;
use serde_json::Value;
use veyndo_catalog::domain::rich_document::safe_url;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::document_slot::field;
use veyndo_core::{Result, bad, conflict};
use veyndo_kernel::verified_kernel;

/// A merchant marks a manual payment; external payments change only on a provider receipt.
/// The order facts manual payment and cancellation transitions check.
#[derive(Deserialize, Default)]
#[serde(default)]
struct TransitionFacts {
    #[serde(deserialize_with = "lenient")]
    state: Option<String>,
    #[serde(deserialize_with = "lenient")]
    payment: PaymentState,
    #[serde(deserialize_with = "lenient")]
    deliveries: Option<Vec<Value>>,
}

/// A payment or delivery state.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PaymentState {
    #[serde(deserialize_with = "lenient")]
    state: Option<String>,
}

pub fn payment(o: &mut Value, machine: &OrderMachine, target: &str) -> Result<()> {
    if external(o) {
        return Err(conflict(
            "External payments require a confirmed provider receipt",
        ));
    }
    let facts: TransitionFacts = view(o);
    let current = facts.payment.state.as_deref().unwrap_or("");
    if !verified_kernel::manual_payment_admissible(
        machine
            .states
            .iter()
            .any(|s| s.id == facts.state.as_deref().unwrap_or("") && s.terminal),
        external(o),
        ["pending", "authorized"].contains(&current),
        target == "paid",
    ) {
        return Err(conflict("Invalid payment transition"));
    }
    merge(field(o, "payment")?, Stated { state: target })
}
/// A requested delivery transition: the shipment and its tracking details.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DeliveryTransition {
    #[serde(deserialize_with = "lenient")]
    delivery_index: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    tracking_code: Option<String>,
    #[serde(deserialize_with = "lenient")]
    tracking_url: Option<String>,
}

/// Ships or delivers one delivery of the order and records safe tracking details.
pub fn delivery(o: &mut Value, v: &Value, target: &str) -> Result<()> {
    let input: DeliveryTransition = view(v);
    let index = input.delivery_index.unwrap_or(0) as usize;
    let d = field(o, "deliveries")?
        .as_array_mut()
        .and_then(|v| v.get_mut(index))
        .ok_or(bad("Order has no delivery"))?;
    let control: DeliveryControl = view(d);
    let current = control.state.as_str().unwrap_or("open");
    if !control.booking.is_null() {
        return Err(conflict("Reconcile the carrier booking before dispatch"));
    }
    if !matches!(
        (current, target),
        ("open", "shipped") | ("shipped", "delivered")
    ) {
        return Err(conflict("Invalid delivery transition"));
    }
    if target == "shipped"
        && let Some(track) = input.tracking_code.as_deref()
    {
        if track.len() > 100 {
            return Err(bad("Tracking code too long"));
        }
        merge(
            &mut *d,
            TrackingCode {
                tracking_code: track,
            },
        )?;
    }
    if let Some(url) = input.tracking_url.as_deref() {
        if !url.is_empty() && (!url.starts_with("https://") || !safe_url(url)) {
            return Err(bad("Tracking link must be a safe HTTPS URL"));
        }
        merge(&mut *d, TrackingUrl::of(url))?;
    }
    merge(d, Stated { state: target })
}

/// Why an order cannot be cancelled natively yet: captured money or shipped deliveries.
pub fn cancellation_blocker(o: &Value) -> Option<&'static str> {
    if external(o)
        || [
            "paid",
            "captured",
            "partially_captured",
            "partially_refunded",
        ]
        .contains(
            &view::<TransitionFacts>(o)
                .payment
                .state
                .as_deref()
                .unwrap_or(""),
        )
    {
        return Some("Reconcile or refund payment before cancellation");
    }
    view::<TransitionFacts>(o)
        .deliveries
        .is_some_and(|ds| {
            ds.iter()
                .any(|d| view::<PaymentState>(d).state.as_deref() != Some("open"))
        })
        .then_some("Shipped orders require a return workflow")
}

#[cfg(test)]
mod tests {
    use super::{cancellation_blocker, delivery, payment};
    use crate::domain::order_machine::default_machine;
    use serde_json::json;

    #[test]
    fn manual_payments_are_marked_paid_but_provider_payments_are_not() {
        let m = default_machine();
        let mut o = json!({"state":"placed","payment":{"provider":"manual","state":"pending"}});
        payment(&mut o, &m, "paid").unwrap();
        assert_eq!(o["payment"]["state"], "paid");
        let mut external =
            json!({"state":"placed","payment":{"provider":"stripe","state":"pending"}});
        assert!(payment(&mut external, &m, "paid").is_err());
        let mut refund = json!({"state":"placed","payment":{"provider":"manual","state":"paid"}});
        assert!(payment(&mut refund, &m, "paid").is_err());
    }
    #[test]
    fn deliveries_ship_once_with_safe_tracking() {
        let mut o = json!({"deliveries":[{"state":"open","booking":null}]});
        assert!(delivery(&mut o, &json!({}), "delivered").is_err());
        assert!(
            delivery(
                &mut o,
                &json!({"trackingUrl":"http://carrier.example/1"}),
                "shipped"
            )
            .is_err()
        );
        delivery(
            &mut o,
            &json!({"trackingCode":"T1","trackingUrl":"https://carrier.example/1"}),
            "shipped",
        )
        .unwrap();
        assert_eq!(o["deliveries"][0]["trackingCode"], "T1");
        delivery(&mut o, &json!({"trackingUrl":""}), "delivered").unwrap();
        assert!(o["deliveries"][0]["trackingUrl"].is_null());
        let mut booked = json!({"deliveries":[{"state":"open","booking":{"id":"b"}}]});
        assert!(delivery(&mut booked, &json!({}), "shipped").is_err());
        assert!(delivery(&mut json!({"deliveries":[]}), &json!({}), "shipped").is_err());
    }
    #[test]
    fn cancellation_needs_open_money_and_deliveries() {
        assert!(cancellation_blocker(&json!({"payment":{"provider":"manual","state":"pending"},"deliveries":[{"state":"open"}]})).is_none());
        assert!(
            cancellation_blocker(&json!({"payment":{"provider":"manual","state":"paid"}}))
                .is_some()
        );
        assert!(
            cancellation_blocker(&json!({"payment":{"provider":"stripe","state":"pending"}}))
                .is_some()
        );
        assert!(cancellation_blocker(&json!({"payment":{"provider":"manual","state":"pending"},"deliveries":[{"state":"shipped"}]})).is_some());
    }
}
