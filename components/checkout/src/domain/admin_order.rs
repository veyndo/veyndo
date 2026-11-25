//! Orders a merchant places for a customer: only manual or simulated payment, and the actor is kept on the record.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Result, bad};
use veyndo_pricing::verified_kernel;
use veyndo_shop::types::cart::StoredCart;

/// The final locked checkout quote must never enter an external provider for admin orders.
/// The payment method of a manual order quote.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ManualQuote {
    #[serde(deserialize_with = "lenient")]
    payment_method: MethodMode,
}

/// The mode of a payment method.
#[derive(Deserialize, Default)]
#[serde(default)]
struct MethodMode {
    mode: Value,
}

pub fn admit_payment(c: &StoredCart, quote: &Value) -> Result<()> {
    let mode = view::<ManualQuote>(quote).payment_method.mode;
    if !verified_kernel::admin_payment_admissible(
        c.data.admin_actor.is_some(),
        mode == "manual",
        mode == "simulated",
    ) {
        return Err(bad(
            "Buyer payment required for this provider; review the configured manual payment again",
        ));
    }
    Ok(())
}
/// Marks the order as created by a merchant.
pub fn order(c: &StoredCart, order: &mut Value) -> Result<()> {
    if let Some(actor) = &c.data.admin_actor {
        merge(
            order,
            MerchantOrder {
                created_by: CreatedBy {
                    source: "merchant",
                    actor,
                },
            },
        )?;
    }
    Ok(())
}

/// Who created an order a merchant placed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MerchantOrder<'a> {
    created_by: CreatedBy<'a>,
}

/// The merchant source and actor of an order.
#[derive(Serialize)]
struct CreatedBy<'a> {
    source: &'static str,
    actor: &'a str,
}
#[cfg(test)]
mod tests {
    use super::{admit_payment, order};
    use serde_json::json;
    use veyndo_shop::types::cart::StoredCart;

    fn cart(admin: Option<&str>) -> StoredCart {
        StoredCart {
            id: "cart-1".into(),
            tenant: "shop".into(),
            token: "token".into(),
            data: serde_json::from_value(json!({"items":[],"group":"retail","session":"s",
                "customer_id":"customer-1","locale":"de-DE","admin_actor":admin}))
            .expect("cart"),
            revision: 3,
            status: "open".into(),
        }
    }

    #[test]
    fn a_merchant_order_never_enters_an_external_provider() {
        let admin = cart(Some("merchant-1"));
        for mode in ["manual", "simulated"] {
            assert!(admit_payment(&admin, &json!({"paymentMethod":{"mode":mode}})).is_ok());
        }
        for mode in ["app", "cash", ""] {
            assert!(admit_payment(&admin, &json!({"paymentMethod":{"mode":mode}})).is_err());
        }
        assert!(admit_payment(&cart(None), &json!({"paymentMethod":{"mode":"app"}})).is_ok());
    }

    #[test]
    fn only_merchant_orders_carry_their_actor() {
        let base = json!({"id":"o1","state":"placed"});
        let mut placed = base.clone();
        order(&cart(Some("merchant-1")), &mut placed).unwrap();
        let mut indexed = base.clone();
        indexed["createdBy"] = json!({"source":"merchant","actor":"merchant-1"});
        assert_eq!(placed, indexed);
        let mut placed = base.clone();
        order(&cart(None), &mut placed).unwrap();
        assert_eq!(placed, base);
    }
}
