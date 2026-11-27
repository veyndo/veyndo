//! The outbox events the inventory component publishes: typed payloads behind `contracts/events/`.
use veyndo_core::event;

event! {
    /// Sandbox test stock filled an empty product at a new product revision.
    #[event(kinds = ["product.updated"], version = 1)]
    pub struct TestStockFilled<'a> {
        product_id: &'a str => "productId",
        revision: i64 => "revision",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[TestStockFilled::SPEC]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        assert_eq!(
            TestStockFilled {
                product_id: "p",
                revision: 2
            }
            .payload()
            .unwrap(),
            json!({"productId":"p","revision":2})
        );
    }
}
