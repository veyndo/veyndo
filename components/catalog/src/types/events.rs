//! The outbox events the catalog component publishes: typed payloads behind `contracts/events/`.
use veyndo_core::event;

event! {
    /// A migration imported a record into this component (`kind`: the record family, here `reviews`).
    #[event(kinds = ["migration.record.imported"], version = 1)]
    pub struct RecordImported<'a> {
        kind: &'a str => "kind",
        target_id: &'a str => "targetId",
    }
}

event! {
    /// An approved translation job wrote a product's texts in one locale.
    #[event(kinds = ["product.updated"], version = 1)]
    pub struct TranslationApplied<'a> {
        product_id: &'a str => "productId",
        locale: &'a str => "locale",
        revision: i64 => "revision",
        job_id: &'a str => "jobId",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[RecordImported::SPEC, TranslationApplied::SPEC]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        assert_eq!(
            RecordImported {
                kind: "reviews",
                target_id: "r"
            }
            .payload()
            .unwrap(),
            json!({"kind":"reviews","targetId":"r"})
        );
        assert_eq!(
            TranslationApplied {
                product_id: "p",
                locale: "de-DE",
                revision: 4,
                job_id: "j"
            }
            .payload()
            .unwrap(),
            json!({"productId":"p","locale":"de-DE","revision":4,"jobId":"j"})
        );
    }
}
