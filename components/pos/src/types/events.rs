//! The outbox events the POS component publishes: typed payloads behind `contracts/events/`.
use serde_json::Value;
use veyndo_core::event;

event! {
    /// A cashier selected (or cleared, `customerId: null`) the customer of a register cart.
    #[event(kinds = ["pos.customer_selected"], version = 1)]
    pub struct CustomerSelected<'a> {
        cart_id: &'a str => "cartId",
        customer_id: Option<&'a str> => "customerId",
        cashier_id: Option<&'a str> => "cashierId",
        sales_channel_id: &'a str => "salesChannelId",
    }
}

event! {
    /// A cashier changed the discount of a register cart; `previous` and `discount` are the cart's discount records or `null`.
    #[event(kinds = ["pos.discount_changed"], version = 1)]
    pub struct DiscountChanged<'a> {
        cart_id: &'a str => "cartId",
        sales_channel_id: &'a str => "salesChannelId",
        cashier_id: Option<&'a str> => "cashierId",
        previous: Value => "previous",
        discount: Value => "discount",
        revision: i64 => "revision",
    }
}

event! {
    /// A register sale was paid in cash at the counter.
    #[event(kinds = ["payment.state_changed"], version = 1)]
    pub struct CashPaid<'a> {
        order_id: Option<&'a str> => "orderId",
        state: &'a str => "state",
        kind: &'a str => "kind",
        cashier_id: &'a str => "cashierId",
        sales_channel_id: &'a str => "salesChannelId",
    }
}

event! {
    /// A register sale was placed: `pos.sale_completed` once paid, otherwise `pos.sale_created`.
    /// `totalMinor` is the order's minor-unit total as the order document stores it.
    #[event(kinds = ["pos.sale_completed", "pos.sale_created"], version = 1)]
    pub struct SalePlaced<'a> {
        order_id: Option<&'a str> => "orderId",
        sales_channel_id: &'a str => "salesChannelId",
        cashier_id: &'a str => "cashierId",
        payment_state: Option<&'a str> => "paymentState",
        total_minor: Value => "totalMinor",
        currency: &'a str => "currency",
    }
}

event! {
    /// A register sale paid through a provider completed when the provider captured it.
    #[event(kinds = ["pos.sale_completed"], version = 1)]
    pub struct SaleCaptured<'a> {
        order_id: Option<&'a str> => "orderId",
        sales_channel_id: Option<&'a str> => "salesChannelId",
        payment_state: &'a str => "paymentState",
        total_minor: Value => "totalMinor",
        currency: Option<&'a str> => "currency",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        CustomerSelected::SPEC,
        DiscountChanged::SPEC,
        CashPaid::SPEC,
        SalePlaced::SPEC,
        SaleCaptured::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let actor = String::from("cashier");
        let cashier = Some(&actor);
        let discount = json!({"percent":10,"reason":"damaged","actor":"cashier"});
        let checks = [
            (
                CustomerSelected {
                    cart_id: "c",
                    customer_id: Some("u"),
                    cashier_id: Some("cashier"),
                    sales_channel_id: "pos",
                }
                .payload(),
                json!({"cartId":"c","customerId":Some("u"),"cashierId":cashier,"salesChannelId":"pos"}),
            ),
            (
                DiscountChanged {
                    cart_id: "c",
                    sales_channel_id: "pos",
                    cashier_id: None,
                    previous: Value::Null,
                    discount: discount.clone(),
                    revision: 2,
                }
                .payload(),
                json!({"cartId":"c","salesChannelId":"pos","cashierId":None::<&String>,"previous":None::<Value>,"discount":Some(discount),"revision":2}),
            ),
            (
                CashPaid {
                    order_id: Some("o"),
                    state: "paid",
                    kind: "payment",
                    cashier_id: "cashier",
                    sales_channel_id: "pos",
                }
                .payload(),
                json!({"orderId":json!("o"),"state":"paid","kind":"payment","cashierId":actor,"salesChannelId":"pos"}),
            ),
            (
                SalePlaced {
                    order_id: Some("o"),
                    sales_channel_id: "pos",
                    cashier_id: "cashier",
                    payment_state: Some("paid"),
                    total_minor: json!(1250),
                    currency: "EUR",
                }
                .payload(),
                json!({"orderId":json!("o"),"salesChannelId":"pos","cashierId":actor,"paymentState":json!("paid"),"totalMinor":json!(1250),"currency":"EUR"}),
            ),
            (
                SaleCaptured {
                    order_id: Some("o"),
                    sales_channel_id: None,
                    payment_state: "captured",
                    total_minor: Value::Null,
                    currency: Some("EUR"),
                }
                .payload(),
                json!({"orderId":json!("o"),"salesChannelId":Value::Null,"paymentState":"captured","totalMinor":Value::Null,"currency":json!("EUR")}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }
}
