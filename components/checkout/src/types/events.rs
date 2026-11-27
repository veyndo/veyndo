//! The outbox events the checkout component publishes: typed payloads behind `contracts/events/`.
use veyndo_core::event;

event! {
    /// Checkout placed an order; `totalMinor` is the order total in minor units of the order currency.
    #[event(kinds = ["order.placed"], version = 1)]
    pub struct OrderPlaced<'a> {
        order_id: &'a str => "orderId",
        total_minor: i64 => "totalMinor",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[OrderPlaced::SPEC]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let (id, minor) = ("o", 1250i64);
        assert_eq!(
            OrderPlaced {
                order_id: id,
                total_minor: minor
            }
            .payload()
            .unwrap(),
            json!({"orderId":id,"totalMinor":minor})
        );
    }
}
