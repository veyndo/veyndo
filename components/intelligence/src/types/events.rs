//! The outbox events the intelligence component publishes: typed payloads behind `contracts/events/`.
use std::collections::HashMap;
use veyndo_core::{event, event_part};

event! {
    /// A merchant marked a hypothesis; marking never starts an experiment.
    #[event(kinds = ["intelligence.decision"], version = 1)]
    pub struct HypothesisDecided<'a> {
        hypothesis_id: &'a str => "hypothesisId",
        state: &'a str => "state",
        experiment_started: bool => "experimentStarted",
    }
}

event! {
    /// A merchant stopped or resumed all agents of the shop; `cancelled` counts the runs that were cancelled.
    #[event(kinds = ["intelligence.agents.controlled"], version = 1)]
    pub struct AgentsControlled<'a> {
        stopped: bool => "stopped",
        revision: i64 => "revision",
        actor: Option<&'a str> => "actor",
        cancelled: u64 => "cancelled",
    }
}

event! {
    /// An autonomy revert restored a product's baseline price at a new product revision.
    #[event(kinds = ["product.updated"], version = 1)]
    pub struct PriceReverted<'a> {
        product_id: &'a str => "productId",
        revision: i64 => "revision",
    }
}

event! {
    /// Autonomous price changes were reverted, by the merchant or automatically.
    #[event(kinds = ["ai.autonomy.reverted"], version = 1)]
    pub struct AutonomyReverted<'a> {
        actor: &'a str => "actor",
        automatic: bool => "automatic",
        product_ids: &'a [String] => "productIds",
    }
}

event! {
    /// A shopper's consent choices per purpose were recorded for a cart and channel.
    #[event(kinds = ["privacy.consent_changed"], version = 1)]
    pub struct ConsentChanged<'a> {
        cart_id: &'a str => "cartId",
        sales_channel_id: &'a str => "salesChannelId",
        choices: &'a HashMap<String, bool> => "choices",
    }
}

event! {
    /// A merchant approved a supplier reorder; `totalMinor` is a decimal string in minor units of `currency`.
    #[event(kinds = ["supply.reorder.approved"], version = 1)]
    pub struct ReorderApproved<'a> {
        product_id: &'a str => "productId",
        supplier_shop: &'a str => "supplierShop",
        quantity: i32 => "quantity",
        total_minor: String => "totalMinor",
        currency: Option<&'a str> => "currency",
        actor: Option<&'a str> => "actor",
    }
}

event! {
    /// A knowledge source was stored for the first time.
    #[event(kinds = ["knowledge.document.ingested"], version = 1)]
    pub struct DocumentIngested<'a> {
        document_id: &'a str => "documentId",
        product_id: Option<&'a str> => "productId",
        source_type: &'a str => "sourceType",
    }
}

event_part! {
    /// The edit of a source: its new revision, private again.
    pub struct Edited<'a> {
        revision: i64 => "revision",
        visibility: &'a str => "visibility",
    }
}

event_part! {
    /// The new revision of an archived or restored source.
    pub struct Revised {
        revision: i64 => "revision",
    }
}

event_part! {
    /// The visibility a source was given.
    pub struct Visible<'a> {
        visibility: &'a str => "visibility",
    }
}

event! {
    /// A merchant edited a knowledge source; it is private again until republished.
    #[event(kinds = ["knowledge.document.updated"], version = 1)]
    pub struct DocumentUpdated<'a> {
        document_id: &'a str => "documentId",
        decision: Edited<'a> => "decision",
    }
}

event! {
    /// A merchant archived or restored a knowledge source.
    #[event(kinds = ["knowledge.document.archived", "knowledge.document.restored"], version = 1)]
    pub struct DocumentLifecycle<'a> {
        document_id: &'a str => "documentId",
        decision: Revised => "decision",
    }
}

event! {
    /// A merchant changed who may read a knowledge source.
    #[event(kinds = ["knowledge.document.visibility"], version = 1)]
    pub struct DocumentVisibility<'a> {
        document_id: &'a str => "documentId",
        decision: Visible<'a> => "decision",
    }
}

event! {
    /// A merchant or integration confirmed or rejected a product claim.
    #[event(kinds = ["intelligence.claim.reviewed"], version = 1)]
    pub struct ClaimReviewed<'a> {
        claim_id: &'a str => "claimId",
        state: &'a str => "state",
        actor: Option<&'a str> => "actor",
    }
}

event! {
    /// An experiment was saved or moved by an operation (start, stop, finish).
    #[event(kinds = ["intelligence.experiment.changed"], version = 1)]
    pub struct ExperimentChanged<'a> {
        id: &'a str => "id",
        operation: &'a str => "operation",
        actor: &'a str => "actor",
    }
}

event! {
    /// Experiment automation decided a settled experiment: the treatment was `published` or `kept`.
    #[event(kinds = ["intelligence.experiment.changed"], version = 1)]
    pub struct ExperimentDecided<'a> {
        id: &'a str => "id",
        operation: &'a str => "operation",
        decision: &'a str => "decision",
        actor: &'a str => "actor",
    }
}

event! {
    /// A mature experiment readout was recorded for its channel.
    #[event(kinds = ["intelligence.experiment.result"], version = 1)]
    pub struct ExperimentResult<'a> {
        id: &'a str => "id",
        channel_id: &'a str => "channelId",
        causal_uplift_proven: Option<bool> => "causalUpliftProven",
        as_of: &'a str => "asOf",
    }
}

event! {
    /// The state of a durable agent run changed; written by the `emit_agent_run_state` trigger of the intelligence baseline.
    #[event(kinds = ["intelligence.run.changed"], version = 1, source = SqlTrigger)]
    pub struct RunChanged<'a> {
        task_id: &'a str => "taskId",
        state: &'a str => "state",
        revision: i64 => "revision",
    }
}

/// The registry entries of this component.
pub const EVENTS: &[veyndo_core::events::Spec] = {
    use veyndo_core::events::Event;
    &[
        HypothesisDecided::SPEC,
        AgentsControlled::SPEC,
        PriceReverted::SPEC,
        AutonomyReverted::SPEC,
        ConsentChanged::SPEC,
        ReorderApproved::SPEC,
        DocumentIngested::SPEC,
        DocumentUpdated::SPEC,
        DocumentLifecycle::SPEC,
        DocumentVisibility::SPEC,
        ClaimReviewed::SPEC,
        ExperimentChanged::SPEC,
        ExperimentDecided::SPEC,
        ExperimentResult::SPEC,
        RunChanged::SPEC,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_core::events::Event;

    #[test]
    fn payloads_equal_the_json_the_producers_stored_before() {
        let ids = vec![String::from("p")];
        let choices: HashMap<String, bool> =
            [("analytics".to_string(), true)].into_iter().collect();
        let checks = [
            (
                HypothesisDecided {
                    hypothesis_id: "h",
                    state: "confirmed",
                    experiment_started: false,
                }
                .payload(),
                json!({"hypothesisId":"h","state":"confirmed","experimentStarted":false}),
            ),
            (
                AgentsControlled {
                    stopped: true,
                    revision: 2,
                    actor: Some("m"),
                    cancelled: 1,
                }
                .payload(),
                json!({"stopped":true,"revision":1+1,"actor":Some("m"),"cancelled":1u64}),
            ),
            (
                PriceReverted {
                    product_id: "p",
                    revision: 4,
                }
                .payload(),
                json!({"productId":"p","revision":4}),
            ),
            (
                AutonomyReverted {
                    actor: "m",
                    automatic: false,
                    product_ids: &ids,
                }
                .payload(),
                json!({"actor":"m","automatic":false,"productIds":ids}),
            ),
            (
                ConsentChanged {
                    cart_id: "c",
                    sales_channel_id: "web",
                    choices: &choices,
                }
                .payload(),
                json!({"cartId":"c","salesChannelId":"web","choices":choices}),
            ),
            (
                ReorderApproved {
                    product_id: "p",
                    supplier_shop: "s",
                    quantity: 5,
                    total_minor: 1250.to_string(),
                    currency: Some("EUR"),
                    actor: None,
                }
                .payload(),
                json!({"productId":"p","supplierShop":"s","quantity":5i32,"totalMinor":"1250","currency":"EUR","actor":None::<&str>}),
            ),
            (
                DocumentIngested {
                    document_id: "d",
                    product_id: None,
                    source_type: "upload",
                }
                .payload(),
                json!({"documentId":"d","productId":None::<&str>,"sourceType":"upload"}),
            ),
            (
                DocumentUpdated {
                    document_id: "d",
                    decision: Edited {
                        revision: 2,
                        visibility: "private",
                    },
                }
                .payload(),
                json!({"documentId":"d","decision":json!({"revision":1+1,"visibility":"private"})}),
            ),
            (
                DocumentLifecycle {
                    document_id: "d",
                    decision: Revised { revision: 3 },
                }
                .payload(),
                json!({"documentId":"d","decision":json!({"revision":3})}),
            ),
            (
                DocumentVisibility {
                    document_id: "d",
                    decision: Visible {
                        visibility: "public",
                    },
                }
                .payload(),
                json!({"documentId":"d","decision":json!({"visibility":"public"})}),
            ),
            (
                ClaimReviewed {
                    claim_id: "c",
                    state: "confirmed",
                    actor: None,
                }
                .payload(),
                json!({"claimId":"c","state":"confirmed","actor":None::<&str>}),
            ),
            (
                ExperimentChanged {
                    id: "e",
                    operation: "knowledge.experiment.finish",
                    actor: "automation",
                }
                .payload(),
                json!({"id":"e","operation":"knowledge.experiment.finish","actor":"automation"}),
            ),
            (
                ExperimentDecided {
                    id: "e",
                    operation: "knowledge.experiment.decide",
                    decision: "kept",
                    actor: "automation",
                }
                .payload(),
                json!({"id":"e","operation":"knowledge.experiment.decide","decision":"kept","actor":"automation"}),
            ),
            (
                ExperimentResult {
                    id: "e",
                    channel_id: "web",
                    causal_uplift_proven: Some(false),
                    as_of: "2026-11-27T00:00:00+00:00",
                }
                .payload(),
                json!({"id":"e","channelId":"web","causalUpliftProven":json!(false),"asOf":"2026-11-27T00:00:00+00:00"}),
            ),
        ];
        for (new, old) in checks {
            assert_eq!(new.unwrap(), old);
        }
    }

    #[test]
    fn the_trigger_builds_the_declared_fields() {
        let baseline = include_str!("../../migrations/0001-baseline.sql");
        assert!(baseline.contains("'intelligence.run.changed',\n   jsonb_build_object('taskId',NEW.id,'state',NEW.run_state,'revision',NEW.run_revision)"));
        let schema = (RunChanged::SPEC.schema)();
        assert_eq!(schema["required"], json!(["revision", "state", "taskId"]));
    }
}
