//! The outbox events the shop component publishes: typed payloads behind `contracts/events/`.
use serde_json::Value;
use veyndo_core::event;

event! {
    /// The shop-wide commerce settings were saved at a new revision.
    #[event(kinds = ["commerce.configured"], version = 1)]
    pub struct CommerceConfigured {
        revision: i64 => "revision",
    }
}

event! {
    /// The settings overrides of one sales channel were saved.
    #[event(kinds = ["commerce.configured"], version = 1)]
    pub struct ChannelConfigured<'a> {
        channel_id: &'a str => "channelId",
        revision: i64 => "revision",
    }
}

event! {
    /// A currency rate refresh saved new exchange rates (`reason`: `currency_rates`).
    #[event(kinds = ["commerce.configured"], version = 1)]
    pub struct RatesRefreshed<'a> {
        revision: i64 => "revision",
        reason: &'a str => "reason",
    }
}

event! {
    /// A shopper filed a consumer-rights request (withdrawal, access, erase, correct, portability, objection);
    /// `receipt` is the stored request record.
    #[event(kinds = ["consumer.{kind}.requested"], version = 1)]
    pub struct ConsumerRequested<'a> {
        request_id: &'a str => "requestId",
        sales_channel_id: &'a str => "salesChannelId",
        kind: &'a str => "kind",
        receipt: &'a Value => "receipt",
    }
}

event! {
    /// A merchant reviewed a consumer-rights request.
    #[event(kinds = ["consumer.request.reviewed"], version = 1)]
    pub struct ConsumerRequestReviewed<'a> {
        request_id: &'a str => "requestId",
        state: &'a str => "state",
        revision: i64 => "revision",
    }
}

event! {
    /// A cached read table changed; written by the `emit_read_cache_event` trigger of the shop baseline.
    #[event(kinds = ["cache.invalidate"], version = 1, source = SqlTrigger)]
    pub struct CacheInvalidated<'a> {
        namespace: &'a str => "namespace",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        CommerceConfigured::SPEC,
        ChannelConfigured::SPEC,
        RatesRefreshed::SPEC,
        ConsumerRequested::SPEC,
        ConsumerRequestReviewed::SPEC,
        CacheInvalidated::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let receipt = json!({"email":"e"});
        let checks = [
            (
                CommerceConfigured { revision: 3 }.payload(),
                json!({"revision":3}),
            ),
            (
                ChannelConfigured {
                    channel_id: "web",
                    revision: 1,
                }
                .payload(),
                json!({"channelId":"web","revision":1}),
            ),
            (
                RatesRefreshed {
                    revision: 2,
                    reason: "currency_rates",
                }
                .payload(),
                json!({"revision":1+1,"reason":"currency_rates"}),
            ),
            (
                ConsumerRequested {
                    request_id: "r",
                    sales_channel_id: "web",
                    kind: "withdrawal",
                    receipt: &receipt,
                }
                .payload(),
                json!({"requestId":"r","salesChannelId":"web","kind":"withdrawal","receipt":receipt}),
            ),
            (
                ConsumerRequestReviewed {
                    request_id: "r",
                    state: "done",
                    revision: 2,
                }
                .payload(),
                json!({"requestId":"r","state":"done","revision":2}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }

    #[test]
    fn the_trigger_builds_the_declared_fields() {
        let baseline = include_str!("../../migrations/0001-baseline.sql");
        assert!(
            baseline.contains("'cache.invalidate',jsonb_build_object('namespace',TG_TABLE_NAME)")
        );
        let schema = (CacheInvalidated::SPEC.schema)();
        assert_eq!(schema["required"], json!(["namespace"]));
    }
}
