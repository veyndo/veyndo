//! The outbox events the apps component publishes: typed payloads behind `contracts/events/`.
//!
//! Events in an app's own namespace (`app.<app>.<event>`) that the app emits, receives by
//! webhook or schedules carry an app-defined payload; the app's manifest declares its shape.
use serde_json::Value;
use std::collections::BTreeSet;
use veyndo_core::event;

event! {
    /// An app emitted an event in its own namespace, received one by webhook or a schedule fired;
    /// the payload is the app's (at most 16 KiB when emitted through the gateway).
    #[event(kinds = ["app.{app}.{event}"], version = 1)]
    pub struct AppDefined(app_defined Value);
}

event! {
    /// A shopper submitted an app form (for example a site form); the entity row holds the fields.
    #[event(kinds = ["app.{app}.submitted"], version = 1)]
    pub struct Submitted<'a> {
        app: &'a str => "app",
        entity: &'a str => "entity",
        id: &'a str => "id",
        intake_key: &'a str => "intakeKey",
    }
}

event! {
    /// An integration app exported a merchant-private knowledge source into the shop.
    #[event(kinds = ["app.{app}.source_imported"], version = 1)]
    pub struct SourceImported<'a> {
        source_id: &'a str => "sourceId",
        app: &'a str => "app",
        source_kind: &'a str => "sourceKind",
        title: &'a str => "title",
        metadata: &'a Value => "metadata",
    }
}

event! {
    /// A merchant re-queued an app's past event deliveries after an outbox ID.
    #[event(kinds = ["app.replay_requested"], version = 1)]
    pub struct ReplayRequested<'a> {
        app: &'a str => "app",
        after: i64 => "after",
        queued: u64 => "queued",
        actor: Option<&'a str> => "actor",
    }
}

event! {
    /// A merchant approved a callback key with the listed permissions for an app package digest.
    #[event(kinds = ["app.callback_approved"], version = 1)]
    pub struct CallbackApproved<'a> {
        app: &'a str => "app",
        digest: &'a str => "digest",
        permissions: Vec<&'a str> => "permissions",
        actor: &'a str => "actor",
        key_id: &'a str => "keyId",
    }
}

event! {
    /// A hosted frontend activated its inactive app package.
    #[event(kinds = ["app.activated"], version = 1)]
    pub struct Activated<'a> {
        app: &'a str => "app",
        reason: &'a str => "reason",
    }
}

event! {
    /// An app configured (or removed) its product configuration in a cart; `products` are the configured product IDs.
    #[event(kinds = ["app.configuration.changed"], version = 1)]
    pub struct ConfigurationChanged<'a> {
        app: &'a str => "app",
        cart_id: &'a str => "cartId",
        revision: i64 => "revision",
        sales_channel_id: &'a str => "salesChannelId",
        operation: &'a str => "operation",
        products: BTreeSet<&'a String> => "products",
    }
}

event! {
    /// An app placed an external order (for example a marketplace sale); `totalMinor` is in minor units.
    #[event(kinds = ["order.placed"], version = 1)]
    pub struct ExternalOrderPlaced<'a> {
        order_id: &'a str => "orderId",
        total_minor: i64 => "totalMinor",
        sales_channel_id: &'a str => "salesChannelId",
        app_id: &'a str => "appId",
    }
}

event! {
    /// A merchant connected a frontend address to a sales channel.
    #[event(kinds = ["frontend.configured"], version = 1)]
    pub struct FrontendConfigured<'a> {
        alias: &'a str => "alias",
        channel: &'a str => "channel",
        revision: i64 => "revision",
        actor: Option<&'a str> => "actor",
    }
}

event! {
    /// A merchant disconnected a frontend address.
    #[event(kinds = ["frontend.disconnected"], version = 1)]
    pub struct FrontendDisconnected<'a> {
        alias: &'a str => "alias",
        actor: Option<&'a str> => "actor",
    }
}

event! {
    /// A carrier app reserved or released the booking of one package of an order.
    #[event(kinds = ["delivery.booking_reserved", "delivery.booking_released"], version = 1)]
    pub struct BookingChanged<'a> {
        order_id: &'a str => "orderId",
        delivery_index: usize => "deliveryIndex",
        source_app: &'a str => "sourceApp",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        AppDefined::SPEC,
        Submitted::SPEC,
        SourceImported::SPEC,
        ReplayRequested::SPEC,
        CallbackApproved::SPEC,
        Activated::SPEC,
        ConfigurationChanged::SPEC,
        ExternalOrderPlaced::SPEC,
        FrontendConfigured::SPEC,
        FrontendDisconnected::SPEC,
        BookingChanged::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let input = json!({"anything":[1,"two"]});
        let metadata = json!({"url":"https://example.test"});
        let permissions = vec!["orders.read"];
        let product = String::from("p");
        let products: BTreeSet<&String> = [&product].into_iter().collect();
        let checks = [
            (AppDefined(input.clone()).payload(), input),
            (
                Submitted {
                    app: "site_forms",
                    entity: "lead",
                    id: "i",
                    intake_key: "k",
                }
                .payload(),
                json!({"app":"site_forms","entity":"lead","id":"i","intakeKey":"k"}),
            ),
            (
                SourceImported {
                    source_id: "s",
                    app: "notion",
                    source_kind: "page",
                    title: "T",
                    metadata: &metadata,
                }
                .payload(),
                json!({"sourceId":"s","app":"notion","sourceKind":"page","title":"T","metadata":metadata}),
            ),
            (
                ReplayRequested {
                    app: "slack",
                    after: 10,
                    queued: 2,
                    actor: Some("m"),
                }
                .payload(),
                json!({"app":"slack","after":10i64,"queued":2u64,"actor":Some("m")}),
            ),
            (
                CallbackApproved {
                    app: "slack",
                    digest: "d",
                    permissions: permissions.clone(),
                    actor: "m",
                    key_id: "k",
                }
                .payload(),
                json!({"app":"slack","digest":"d","permissions":permissions,"actor":"m","keyId":"k"}),
            ),
            (
                Activated {
                    app: "a",
                    reason: "hosted_frontend",
                }
                .payload(),
                json!({"app":"a","reason":"hosted_frontend"}),
            ),
            (
                ConfigurationChanged {
                    app: "a",
                    cart_id: "c",
                    revision: 2,
                    sales_channel_id: "web",
                    operation: "configured",
                    products: products.clone(),
                }
                .payload(),
                json!({"app":"a","cartId":"c","revision":2,"salesChannelId":"web","operation":"configured","products":products}),
            ),
            (
                ExternalOrderPlaced {
                    order_id: "o",
                    total_minor: 990,
                    sales_channel_id: "market",
                    app_id: "marketplaces",
                }
                .payload(),
                json!({"orderId":"o","totalMinor":990i64,"salesChannelId":"market","appId":"marketplaces"}),
            ),
            (
                FrontendConfigured {
                    alias: "a",
                    channel: "web",
                    revision: 1,
                    actor: None,
                }
                .payload(),
                json!({"alias":"a","channel":"web","revision":1,"actor":None::<&str>}),
            ),
            (
                FrontendDisconnected {
                    alias: "a",
                    actor: Some("m"),
                }
                .payload(),
                json!({"alias":"a","actor":Some("m")}),
            ),
            (
                BookingChanged {
                    order_id: "o",
                    delivery_index: 0,
                    source_app: "dhl",
                }
                .payload(),
                json!({"orderId":"o","deliveryIndex":0usize,"sourceApp":"dhl"}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }
}
