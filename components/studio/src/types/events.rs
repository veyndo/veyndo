//! The outbox events the studio component publishes: typed payloads behind `contracts/events/`.
use veyndo_core::event;

event! {
    /// The merchant product editor created (`product.created`) or saved (`product.updated`) a product.
    #[event(kinds = ["product.created", "product.updated"], version = 1)]
    pub struct ProductSaved<'a> {
        product_id: &'a str => "productId",
        revision: i64 => "revision",
    }
}

event! {
    /// A staging release published a product to the live shop.
    #[event(kinds = ["product.published"], version = 1)]
    pub struct ProductPublished<'a> {
        product_id: &'a str => "productId",
        revision: i64 => "revision",
    }
}

event! {
    /// The merchant saved the order workflow (state machine); `sourceApp` names an app that supplied it.
    #[event(kinds = ["order.workflow.updated"], version = 1)]
    pub struct WorkflowUpdated<'a> {
        revision: i64 => "revision",
        actor: Option<&'a str> => "actor",
        source_app: Option<&'a str> => "sourceApp",
    }
}

event! {
    /// A migration imported a historical order (`kind`: `orders`).
    #[event(kinds = ["migration.record.imported"], version = 1)]
    pub struct RecordImported<'a> {
        kind: &'a str => "kind",
        target_id: &'a str => "targetId",
    }
}

event! {
    /// A reviewed proposal (task) was applied to the shop, by the merchant or autonomously.
    #[event(kinds = ["merchant.change.applied"], version = 1)]
    pub struct ChangeApplied<'a> {
        task_id: &'a str => "taskId",
        autonomous: bool => "autonomous",
        actor: Option<&'a str> => "actor",
    }
}

event! {
    /// A staging release published the selected units (`selections`: their keys) to the live shop.
    #[event(kinds = ["shop.release.published"], version = 1)]
    pub struct ReleasePublished<'a> {
        release_id: &'a str => "releaseId",
        selections: &'a [String] => "selections",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        ProductSaved::SPEC,
        ProductPublished::SPEC,
        WorkflowUpdated::SPEC,
        RecordImported::SPEC,
        ChangeApplied::SPEC,
        ReleasePublished::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let keys = vec![String::from("product:p"), String::from("settings")];
        let source_app: Option<String> = None;
        let checks = [
            (
                ProductSaved {
                    product_id: "p",
                    revision: 3,
                }
                .payload(),
                json!({"productId":"p","revision":2+1}),
            ),
            (
                ProductPublished {
                    product_id: "p",
                    revision: 3,
                }
                .payload(),
                json!({"productId":"p","revision":3}),
            ),
            (
                WorkflowUpdated {
                    revision: 2,
                    actor: Some("m"),
                    source_app: source_app.as_deref(),
                }
                .payload(),
                json!({"revision":2,"actor":Some("m"),"sourceApp":source_app}),
            ),
            (
                RecordImported {
                    kind: "orders",
                    target_id: "o",
                }
                .payload(),
                json!({"kind":"orders","targetId":"o"}),
            ),
            (
                ChangeApplied {
                    task_id: "t",
                    autonomous: false,
                    actor: None,
                }
                .payload(),
                json!({"taskId":"t","autonomous":false,"actor":None::<&str>}),
            ),
            (
                ReleasePublished {
                    release_id: "r",
                    selections: &keys,
                }
                .payload(),
                json!({"releaseId":"r","selections":json!(keys)}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }
}
