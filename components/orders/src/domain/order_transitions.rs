//! Revision-free order transition rules: manual payment marks, delivery dispatch and the cancellation preconditions.
use crate::domain::order_machine::OrderMachine;
use crate::domain::order_workflow::external;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_catalog::domain::rich_document::safe_url;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad, conflict};
use veyndo_kernel::verified_kernel;

/// A merchant marks a manual payment; external payments change only on a provider receipt.
pub fn payment(o: &mut Value, machine: &OrderMachine, target: &str) -> Result<()> {
    if external(o) {
        return Err(conflict(
            "External payments require a confirmed provider receipt",
        ));
    }
    let current = o["payment"]["state"].as_str().unwrap_or("");
    if !verified_kernel::manual_payment_admissible(
        machine
            .states
            .iter()
            .any(|s| s.id == o["state"].as_str().unwrap_or("") && s.terminal),
        external(o),
        ["pending", "authorized"].contains(&current),
        target == "paid",
    ) {
        return Err(conflict("Invalid payment transition"));
    }
    o["payment"]["state"] = json!(target);
    Ok(())
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
    let d = o["deliveries"]
        .as_array_mut()
        .and_then(|v| v.get_mut(index))
        .ok_or(bad("Order has no delivery"))?;
    let current = d["state"].as_str().unwrap_or("open");
    if !d["booking"].is_null() {
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
        d["trackingCode"] = json!(track);
    }
    if let Some(url) = input.tracking_url.as_deref() {
        if !url.is_empty() && (!url.starts_with("https://") || !safe_url(url)) {
            return Err(bad("Tracking link must be a safe HTTPS URL"));
        }
        d["trackingUrl"] = if url.is_empty() {
            Value::Null
        } else {
            json!(url)
        };
    }
    d["state"] = json!(target);
    Ok(())
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
        .contains(&o["payment"]["state"].as_str().unwrap_or(""))
    {
        return Some("Reconcile or refund payment before cancellation");
    }
    o["deliveries"]
        .as_array()
        .is_some_and(|ds| ds.iter().any(|d| d["state"] != "open"))
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
