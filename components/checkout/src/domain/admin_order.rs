//! Orders a merchant places for a customer: only manual or simulated payment, and the actor is kept on the record.
use serde_json::{Value, json};
use veyndo_core::{Result, bad};
use veyndo_pricing::verified_kernel;
use veyndo_shop::types::cart::StoredCart;

/// The final locked checkout quote must never enter an external provider for admin orders.
pub fn admit_payment(c: &StoredCart, quote: &Value) -> Result<()> {
    if !verified_kernel::admin_payment_admissible(
        c.data.admin_actor.is_some(),
        quote["paymentMethod"]["mode"] == "manual",
        quote["paymentMethod"]["mode"] == "simulated",
    ) {
        return Err(bad(
            "Buyer payment required for this provider; review the configured manual payment again",
        ));
    }
    Ok(())
}
/// Marks the purchase record with the merchant who placed it.
pub fn purchase(c: &StoredCart, purchase: &mut Value) {
    if let Some(actor) = &c.data.admin_actor {
        purchase["adminActor"] = json!(actor);
        purchase["adminCustomer"] = json!(c.data.customer_id);
    }
}
/// Marks the order as created by a merchant.
pub fn order(c: &StoredCart, order: &mut Value) {
    if let Some(actor) = &c.data.admin_actor {
        order["createdBy"] = json!({"source":"merchant","actor":actor});
    }
}
