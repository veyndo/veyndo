//! The outbox events the orders component publishes: typed payloads behind `contracts/events/`.
use serde_json::Value;
use veyndo_core::event;

event! {
    /// An order mutation was recorded: the audited change kind, its target state and the new order revision.
    #[event(kinds = ["order.updated"], version = 1)]
    pub struct OrderUpdated<'a> {
        order_id: &'a str => "orderId",
        kind: Option<&'a str> => "kind",
        state: Option<&'a str> => "state",
        revision: Option<i64> => "revision",
    }
}

event! {
    /// The order, its payment or one delivery moved to a new state; `order` is the order snapshot without the cart token.
    #[event(kinds = ["order.state_changed", "payment.state_changed", "delivery.state_changed"], version = 1)]
    pub struct StateChanged<'a> {
        order_id: &'a str => "orderId",
        kind: &'a str => "kind",
        from: Option<&'a str> => "from",
        state: &'a str => "state",
        revision: i64 => "revision",
        request_key: Option<&'a str> => "requestKey",
        order: Value => "order",
    }
}

event! {
    /// Positions of one delivery were split into a new parcel; `order` is the snapshot without the cart token.
    #[event(kinds = ["delivery.split"], version = 1)]
    pub struct DeliverySplit<'a> {
        order_id: &'a str => "orderId",
        delivery_id: &'a str => "deliveryId",
        source_index: usize => "sourceIndex",
        positions: Value => "positions",
        kind: &'a str => "kind",
        state: &'a str => "state",
        revision: u64 => "revision",
        request_key: &'a str => "requestKey",
        order: Value => "order",
    }
}

event! {
    /// A receipt (invoice, delivery note, cancellation) was issued for an order.
    #[event(kinds = ["document.created"], version = 1)]
    pub struct ReceiptCreated<'a> {
        order_id: &'a str => "orderId",
        document_id: &'a str => "documentId",
        kind: &'a str => "kind",
    }
}

event! {
    /// A carrier app stored a shipping or return document for a package of an order.
    #[event(kinds = ["document.created"], version = 1)]
    pub struct CarrierDocumentCreated<'a> {
        order_id: &'a str => "orderId",
        document_id: &'a str => "documentId",
        kind: &'a str => "kind",
        source_app: &'a str => "sourceApp",
    }
}

event! {
    /// The seller master data of the shop or one sales channel was saved.
    #[event(kinds = ["settings.master_data_changed"], version = 1)]
    pub struct MasterDataSaved<'a> {
        revision: i64 => "revision",
        sales_channel_id: Option<&'a str> => "salesChannelId",
    }
}

event! {
    /// A staging release published seller master data to the live shop.
    #[event(kinds = ["settings.master_data_changed"], version = 1)]
    pub struct MasterDataReleased<'a> {
        release: &'a str => "release",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        OrderUpdated::SPEC,
        StateChanged::SPEC,
        DeliverySplit::SPEC,
        ReceiptCreated::SPEC,
        CarrierDocumentCreated::SPEC,
        MasterDataSaved::SPEC,
        MasterDataReleased::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let order = json!({"id":"o","state":"open"});
        let checks = [
            (
                OrderUpdated {
                    order_id: "o",
                    kind: Some("order"),
                    state: Some("paid"),
                    revision: Some(3),
                }
                .payload(),
                json!({"orderId":"o","kind":json!("order"),"state":json!("paid"),"revision":json!(3)}),
            ),
            (
                OrderUpdated {
                    order_id: "o",
                    kind: None,
                    state: None,
                    revision: None,
                }
                .payload(),
                json!({"orderId":"o","kind":Value::Null,"state":Value::Null,"revision":Value::Null}),
            ),
            (
                StateChanged {
                    order_id: "o",
                    kind: "payment",
                    from: Some("open"),
                    state: "paid",
                    revision: 4,
                    request_key: None,
                    order: order.clone(),
                }
                .payload(),
                json!({"orderId":"o","kind":"payment","from":json!("open"),"state":"paid","revision":3+1,"requestKey":None::<&str>,"order":order}),
            ),
            (
                DeliverySplit {
                    order_id: "o",
                    delivery_id: "d",
                    source_index: 0,
                    positions: json!([{"id":"p","quantity":1}]),
                    kind: "delivery",
                    state: "split",
                    revision: 2,
                    request_key: "key",
                    order: order.clone(),
                }
                .payload(),
                json!({"orderId":"o","deliveryId":"d","sourceIndex":0usize,"positions":[{"id":"p","quantity":1}],"kind":"delivery","state":"split","revision":1u64+1,"requestKey":"key","order":order}),
            ),
            (
                ReceiptCreated {
                    order_id: "o",
                    document_id: "r",
                    kind: "invoice",
                }
                .payload(),
                json!({"orderId":"o","documentId":"r","kind":"invoice"}),
            ),
            (
                CarrierDocumentCreated {
                    order_id: "o",
                    document_id: "r",
                    kind: "shipping_label",
                    source_app: "dhl",
                }
                .payload(),
                json!({"orderId":"o","documentId":"r","kind":"shipping_label","sourceApp":"dhl"}),
            ),
            (
                MasterDataSaved {
                    revision: 2,
                    sales_channel_id: Some("web"),
                }
                .payload(),
                json!({"revision":1+1,"salesChannelId":Some("web")}),
            ),
            (
                MasterDataReleased { release: "company" }.payload(),
                json!({"release":"company"}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }
}
