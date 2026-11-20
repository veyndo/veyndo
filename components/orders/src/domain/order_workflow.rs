//! One server-derived action catalogue drives UI, HTTP and MCP; built-in business guards cannot be bypassed.
use crate::domain::order_machine::OrderMachine;
use serde_json::{Value, json};
use veyndo_core::RequestContext;
use veyndo_identity::permit;
use veyndo_pricing::verified_kernel;
pub fn guard(o: &Value, m: &OrderMachine, kind: &str, target: &str) -> Option<&'static str> {
    let state = o["state"].as_str().unwrap_or("");
    let terminal = m.states.iter().any(|s| s.id == state && s.terminal)
        || ["expired", "payment_review"].contains(&state);
    if !verified_kernel::order_edit_admissible(terminal) {
        return Some("terminalOrder");
    }
    let external = external(o);
    let ps = o["payment"]["state"].as_str().unwrap_or("");
    let refund = [
        "paid",
        "captured",
        "partially_captured",
        "partially_refunded",
    ]
    .contains(&ps);
    let deliveries_open = o["deliveries"]
        .as_array()
        .is_none_or(|ds| ds.iter().all(|d| d["state"] == "open"));
    let delivered = o["deliveries"]
        .as_array()
        .is_none_or(|ds| ds.iter().all(|d| d["state"] == "delivered"));
    let payment_ready = ["paid", "captured"].contains(&ps)
        || o["payment"]["provider"] == "simulated" && ps == "authorized";
    match (kind, target) {
        ("order", "cancelled")
            if !verified_kernel::cancellation_admissible(
                terminal,
                external,
                refund,
                deliveries_open,
            ) =>
        {
            Some(if external || refund {
                "refundFirst"
            } else {
                "returnFirst"
            })
        }
        ("order", "completed")
            if !verified_kernel::completion_admissible(terminal, payment_ready, delivered) =>
        {
            Some(if !payment_ready {
                "paymentFirst"
            } else {
                "deliveryFirst"
            })
        }
        ("payment", _) if external => Some("providerConfirmation"),
        _ => None,
    }
}
pub fn workflow(o: &Value, m: &OrderMachine, revision: i64, h: &RequestContext) -> Value {
    let mut actions = Vec::new();
    let current = o["state"].as_str().unwrap_or("placed");
    if m.states.iter().any(|s| s.id == current && s.terminal) {
        return json!({"revision":revision,"mainLocale":o["nativeLifecycle"]["mainLocale"],"states":m.states,"actions":[]});
    }
    if !m.states.iter().any(|s| s.id == current && s.terminal) {
        for e in m.transitions.iter().filter(|e| e.from == current) {
            let reason = guard(o, m, "order", &e.to).or_else(|| {
                veyndo_app_model::native_lifecycle::reason(e, o, |p| permit(h, p).is_ok())
            });
            actions.push(json!({"id":e.id,"kind":"order","from":e.from,"state":e.to,"label":e.label,"enabled":reason.is_none(),"reason":reason,"source":e.policy}));
        }
    }
    if ["pending", "authorized"].contains(&o["payment"]["state"].as_str().unwrap_or(""))
        && !external(o)
    {
        let reason = guard(o, m, "payment", "paid");
        actions.push(json!({"id":"mark_paid","kind":"payment","from":o["payment"]["state"],"state":"paid","enabled":reason.is_none(),"reason":reason}));
    }
    if let Some(ds) = o["deliveries"].as_array() {
        for (index, d) in ds.iter().enumerate() {
            let next = match d["state"].as_str() {
                Some("open") => Some("shipped"),
                Some("shipped") => Some("delivered"),
                _ => None,
            };
            if let Some(next) = next {
                let reason = if !d["booking"].is_null() {
                    Some("carrierBooking")
                } else {
                    guard(o, m, "delivery", next)
                };
                actions.push(json!({"id":format!("delivery_{index}_{next}"),"kind":"delivery","from":d["state"],"state":next,"deliveryIndex":index,"enabled":reason.is_none(),"reason":reason}));
            }
        }
    }
    let splits = o["deliveries"].as_array().map(|ds| ds.iter().enumerate().map(|(index,d)| {
        let quantity = d["positions"].as_array().map(|ps| ps.iter().filter_map(|p| p["quantity"].as_u64()).sum::<u64>()).unwrap_or(0);
        json!({"deliveryIndex":index,"enabled":verified_kernel::delivery_split_admissible(d["state"]=="open",guard(o,m,"delivery","shipped").is_some(),quantity,1) && d["label"].is_null() && d["booking"].is_null() && ds.len()<30})
    }).collect::<Vec<_>>()).unwrap_or_default();
    json!({"revision":revision,"mainLocale":o["nativeLifecycle"]["mainLocale"],"states":m.states,"actions":actions,"shipmentSplits":splits})
}

/// Whether an external payment provider holds the money of this order; manual and simulated payments do not.
pub fn external(o: &Value) -> bool {
    !matches!(
        o["payment"]["provider"].as_str(),
        Some("manual" | "simulated") | None
    )
}

#[cfg(test)]
mod tests {
    use super::{external, guard, workflow};
    use crate::domain::order_machine::default_machine;
    use serde_json::{Value, json};
    use veyndo_core::RequestContext;

    fn order(state: &str, provider: &str, payment: &str, delivery: &str) -> Value {
        json!({"state":state,"payment":{"provider":provider,"state":payment},
            "deliveries":[{"state":delivery,"positions":[{"quantity":2}]}]})
    }

    #[test]
    fn only_another_provider_holds_the_money() {
        assert!(external(&json!({"payment":{"provider":"paypal"}})));
        for provider in [json!("manual"), json!("simulated"), Value::Null] {
            assert!(!external(&json!({"payment":{"provider":provider}})));
        }
    }

    #[test]
    fn business_guards_name_what_has_to_happen_first() {
        let m = default_machine();
        assert_eq!(
            guard(
                &order("completed", "manual", "paid", "delivered"),
                &m,
                "order",
                "cancelled"
            ),
            Some("terminalOrder")
        );
        assert_eq!(
            guard(
                &order("expired", "manual", "pending", "open"),
                &m,
                "payment",
                "paid"
            ),
            Some("terminalOrder")
        );
        assert_eq!(
            guard(
                &order("placed", "manual", "pending", "open"),
                &m,
                "order",
                "cancelled"
            ),
            None
        );
        assert_eq!(
            guard(
                &order("placed", "manual", "paid", "open"),
                &m,
                "order",
                "cancelled"
            ),
            Some("refundFirst")
        );
        assert_eq!(
            guard(
                &order("placed", "paypal", "pending", "open"),
                &m,
                "order",
                "cancelled"
            ),
            Some("refundFirst")
        );
        assert_eq!(
            guard(
                &order("placed", "manual", "pending", "shipped"),
                &m,
                "order",
                "cancelled"
            ),
            Some("returnFirst")
        );
        assert_eq!(
            guard(
                &order("in_progress", "manual", "pending", "delivered"),
                &m,
                "order",
                "completed"
            ),
            Some("paymentFirst")
        );
        assert_eq!(
            guard(
                &order("in_progress", "manual", "paid", "shipped"),
                &m,
                "order",
                "completed"
            ),
            Some("deliveryFirst")
        );
        assert_eq!(
            guard(
                &order("in_progress", "manual", "paid", "delivered"),
                &m,
                "order",
                "completed"
            ),
            None
        );
        assert_eq!(
            guard(
                &order("in_progress", "simulated", "authorized", "delivered"),
                &m,
                "order",
                "completed"
            ),
            None
        );
        assert_eq!(
            guard(
                &order("placed", "paypal", "pending", "open"),
                &m,
                "payment",
                "paid"
            ),
            Some("providerConfirmation")
        );
        assert_eq!(
            guard(
                &order("placed", "manual", "pending", "open"),
                &m,
                "payment",
                "paid"
            ),
            None
        );
        assert_eq!(
            guard(
                &order("placed", "manual", "pending", "open"),
                &m,
                "delivery",
                "shipped"
            ),
            None
        );
    }

    fn action<'a>(w: &'a Value, id: &str) -> Option<&'a Value> {
        w["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == id)
    }

    #[test]
    fn the_catalogue_lists_enabled_and_blocked_actions_of_an_open_order() {
        let m = default_machine();
        let h = RequestContext::new();
        let w = workflow(&order("placed", "manual", "pending", "open"), &m, 7, &h);
        assert_eq!(w["revision"], 7);
        assert_eq!(w["states"].as_array().unwrap().len(), m.states.len());
        assert_eq!(action(&w, "mark_paid").unwrap()["enabled"], true);
        let ship = action(&w, "delivery_0_shipped").unwrap();
        assert_eq!(
            (ship["enabled"].clone(), ship["deliveryIndex"].clone()),
            (json!(true), json!(0))
        );
        assert!(
            w["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["kind"] == "order" && a["from"] == "placed")
        );
        assert_eq!(
            w["shipmentSplits"],
            json!([{"deliveryIndex":0,"enabled":true}])
        );

        let provider = workflow(&order("placed", "paypal", "pending", "open"), &m, 1, &h);
        assert!(action(&provider, "mark_paid").is_none());
        let mut booked = order("placed", "manual", "paid", "shipped");
        booked["deliveries"][0]["booking"] = json!({"carrier":"dhl"});
        let w = workflow(&booked, &m, 1, &h);
        assert!(action(&w, "mark_paid").is_none());
        let deliver = action(&w, "delivery_0_delivered").unwrap();
        assert_eq!(
            (deliver["enabled"].clone(), deliver["reason"].clone()),
            (json!(false), json!("carrierBooking"))
        );
        assert_eq!(w["shipmentSplits"][0]["enabled"], false);
    }

    #[test]
    fn a_finished_order_offers_no_actions() {
        let w = workflow(
            &order("cancelled", "manual", "pending", "open"),
            &default_machine(),
            3,
            &RequestContext::new(),
        );
        assert_eq!(w["actions"], json!([]));
        assert!(w.get("shipmentSplits").is_none());
    }
}
