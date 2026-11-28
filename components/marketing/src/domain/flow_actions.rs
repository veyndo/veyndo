//! Native action schema, permissions and typed facts use original Core names; no arbitrary SQL, shell or unguarded payment transitions.
use crate::domain::flow_text;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_core::document::view;
use veyndo_core::operation_facts::{
    Audience::Merchant, Entity as E, OperationFacts as F, Reversibility, Risk, SideEffect as S,
};
use veyndo_core::{Result, bad};
use veyndo_flow_graph::catalog::{FLOW_ACTIONS, PIPELINE_ACTIONS};

const ASSISTED: &[&str] = &[
    "catalog.write",
    "assistant.use",
    "catalog.read",
    "orders.read",
    "knowledge.read",
];
const fn tag(permission: &'static [&'static str], by: &'static str, on: &'static [E]) -> F {
    F::write(
        Merchant,
        permission,
        Reversibility::Compensatable { by },
        Risk::Low,
    )
    .on(on)
}
const fn field(permission: &'static [&'static str], on: &'static [E]) -> F {
    F::write(Merchant, permission, Reversibility::Reversible, Risk::Low).on(on)
}
const CUSTOMERS: &[&str] = &["customers.write"];
const ORDERS: &[&str] = &["orders.write"];

/// Typed facts of every built-in Flow action: the pipeline actions in published order, then the Flow-only actions.
/// The first permission is the action's native right; the rest are the rights its queued work also needs.
pub const ACTIONS: &[(&str, F)] = &[
    ("note", field(&["automation.manage"], &[E::Automation])),
    (
        "ai_proposal",
        F::write(Merchant, ASSISTED, Reversibility::Reversible, Risk::Low)
            .on(&[E::Run, E::Product])
            .effects(&[S::ModelCall, S::BackgroundJob]),
    ),
    (
        "knowledge.extract",
        F::write(
            Merchant,
            &["knowledge.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Knowledge])
        .effects(&[S::ModelCall]),
    ),
    (
        "app_action",
        F::write(
            Merchant,
            &["apps.manage"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::App])
        .effects(&[S::ExternalCall]),
    ),
    (
        "action.add.customer.tag",
        tag(CUSTOMERS, "action.remove.customer.tag", &[E::Customer]),
    ),
    (
        "action.remove.customer.tag",
        tag(CUSTOMERS, "action.add.customer.tag", &[E::Customer]),
    ),
    (
        "action.add.order.tag",
        tag(ORDERS, "action.remove.order.tag", &[E::Order]),
    ),
    (
        "action.remove.order.tag",
        tag(ORDERS, "action.add.order.tag", &[E::Order]),
    ),
    (
        "action.set.customer.custom.field",
        field(CUSTOMERS, &[E::Customer]),
    ),
    ("action.set.order.custom.field", field(ORDERS, &[E::Order])),
    (
        "action.set.customer.group.custom.field",
        field(CUSTOMERS, &[E::Customer]),
    ),
    (
        "action.add.customer.affiliate.and.campaign.code",
        field(CUSTOMERS, &[E::Customer]),
    ),
    (
        "action.add.order.affiliate.and.campaign.code",
        field(ORDERS, &[E::Order]),
    ),
    (
        "action.change.customer.group",
        F::write(Merchant, CUSTOMERS, Reversibility::Reversible, Risk::Medium)
            .on(&[E::Customer])
            .money(),
    ),
    (
        "action.change.customer.status",
        F::write(Merchant, CUSTOMERS, Reversibility::Reversible, Risk::Medium).on(&[E::Customer]),
    ),
    (
        "action.set.order.state",
        F::write(Merchant, ORDERS, Reversibility::Irreversible, Risk::High)
            .on(&[E::Order, E::Payment])
            .money()
            .stock()
            .effects(&[S::Event]),
    ),
    (
        "action.generate.document",
        F::write(
            Merchant,
            &["documents.create"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Document, E::Order])
        .money()
        .effects(&[S::Event]),
    ),
    (
        "action.grant.download.access",
        F::write(Merchant, ORDERS, Reversibility::Reversible, Risk::Medium).on(&[E::Order]),
    ),
    (
        "action.mail.send",
        F::write(
            Merchant,
            &["apps.manage"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Customer, E::Order])
        .pii()
        .effects(&[S::Email]),
    ),
    (
        "action.stop.flow",
        F::read(Merchant, &["automation.manage"]).on(&[E::Automation]),
    ),
    (
        "work_proposal",
        F::write(Merchant, ASSISTED, Reversibility::Reversible, Risk::Low)
            .on(&[E::Run])
            .effects(&[S::ModelCall, S::BackgroundJob]),
    ),
    ("pipeline", field(ORDERS, &[E::Automation])),
];

const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
const fn listed(names: &[&str], name: &str) -> bool {
    let mut i = 0;
    while i < names.len() {
        if same(names[i], name) {
            return true;
        }
        i += 1;
    }
    false
}
/// The build fails unless the facts cover exactly the published pipeline actions, in order, then the Flow-only actions.
const fn covers(actions: &[(&str, F)], pipeline: &[&str], flow: &[&str]) -> bool {
    if actions.len() < pipeline.len() {
        return false;
    }
    let mut i = 0;
    while i < pipeline.len() {
        if !same(actions[i].0, pipeline[i]) {
            return false;
        }
        i += 1;
    }
    let mut extra = 0;
    while i < actions.len() {
        if listed(pipeline, actions[i].0) || !listed(flow, actions[i].0) {
            return false;
        }
        extra += 1;
        i += 1;
    }
    let mut flow_only = 0;
    let mut j = 0;
    while j < flow.len() {
        if !listed(pipeline, flow[j]) {
            flow_only += 1;
        }
        j += 1;
    }
    extra == flow_only
}
const _: () = assert!(
    covers(ACTIONS, PIPELINE_ACTIONS, FLOW_ACTIONS),
    "every built-in Flow action needs typed facts in flow_actions::ACTIONS"
);

/// The typed facts of a built-in Flow action.
pub fn facts(action: &str) -> Option<&'static F> {
    ACTIONS
        .iter()
        .find(|(name, _)| *name == action)
        .map(|(_, facts)| facts)
}
/// Every right the publishing actor needs for an action; validation rejects unknown actions before this.
pub fn permissions(action: &str) -> &'static [&'static str] {
    facts(action).map_or(ORDERS, |facts| facts.permissions)
}
/// The native right an action is published under.
pub fn permission(action: &str) -> &'static str {
    permissions(action)[0]
}
/// The fields of a flow action's configuration that validation and app steps read.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ActionConfig {
    pub instruction: Value,
    pub tags: Value,
    pub field: Value,
    pub group_id: Value,
    pub active: Value,
    pub value: Value,
    pub kind: Value,
    pub app: Value,
    pub action: Value,
    pub arguments: Value,
    pub template_id: Value,
}

pub fn normalize(action: &str, config: &Value) -> Value {
    let mut c = config.clone();
    for (source, native) in [
        ("tagIds", "tags"),
        ("customerGroupId", "groupId"),
        ("documentType", "kind"),
    ] {
        if c.get(native).is_none() && c.get(source).is_some() {
            c[native] = c[source].clone();
        }
    }
    if action == "action.grant.download.access"
        && c.get("value").is_none()
        && let Some(fields) = c.as_object_mut()
    {
        fields.insert("value".into(), json!(true));
    }
    c
}
pub fn validate(action: &str, config: &Value) -> Result<()> {
    let normalized = normalize(action, config);
    let config = &normalized;
    let fields: ActionConfig = view(config);
    if !PIPELINE_ACTIONS.contains(&action) || !config.is_object() || config.to_string().len() > 8000
    {
        return Err(bad("Unknown flow action or invalid configuration"));
    }
    if matches!(action, "note" | "ai_proposal") {
        flow_text::shape(&fields.instruction)?;
    }
    if action == "knowledge.extract"
        && config.as_object().unwrap().iter().any(|(key, v)| {
            !["sourceId", "productId"].contains(&key.as_str())
                || v.as_str().is_none_or(|s| s.len() > 254)
        })
    {
        return Err(bad(
            "Extraction accepts optional sourceId/productId strings only",
        ));
    }
    let require = |key: &str| {
        config[key]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 254)
            .ok_or(bad(format!("Flow action requires {key}")))
    };
    if action.ends_with(".tag")
        && fields.tags.as_array().is_none_or(|a| {
            a.is_empty()
                || a.len() > 50
                || a.iter()
                    .any(|v| v.as_str().is_none_or(|s| s.is_empty() || s.len() > 100))
        })
    {
        return Err(bad("Flow tags required"));
    }
    if action.contains("custom.field") {
        require("field")?;
        if fields
            .field
            .as_str()
            .unwrap()
            .contains(['.', '/', '[', ']'])
            || config.get("value").is_none()
        {
            return Err(bad("Custom field key and value required"));
        }
    }
    if action == "action.change.customer.group" {
        require("groupId")?;
        if !identifier(fields.group_id.as_str().unwrap()) {
            return Err(bad("Unknown native customer group"));
        }
    }
    if action == "action.change.customer.status" && !fields.active.is_boolean() {
        return Err(bad("Customer active flag required"));
    }
    if action == "action.grant.download.access" && !fields.value.is_boolean() {
        return Err(bad("Download access boolean required"));
    }
    if action == "action.set.order.state" {
        require("kind")?;
        require("state")?;
    }
    if action == "action.generate.document" {
        require("kind")?;
        if !["invoice", "delivery_note"].contains(&fields.kind.as_str().unwrap()) {
            return Err(bad("Unsupported automatic document kind"));
        }
    }
    if action == "app_action" {
        require("app")?;
        require("action")?;
        if !fields.arguments.is_object() {
            return Err(bad("App arguments object required"));
        }
    }
    if action == "action.mail.send" {
        require("templateId")?;
        if fields.template_id != "order_confirmation" {
            return Err(bad("Unknown installed email template"));
        }
    }
    if action.ends_with("affiliate.and.campaign.code") {
        require("affiliateCode")?;
        require("campaignCode")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ACTIONS, facts, normalize, permission, permissions, validate};
    use serde_json::json;

    #[test]
    fn every_action_has_consistent_facts_and_compensations_name_writing_actions() {
        for (name, facts) in ACTIONS {
            facts.check(name).unwrap();
            if let Some(by) = facts.compensation() {
                assert!(super::facts(by).is_some_and(|other| other.writes), "{name}");
            }
        }
        assert!(facts("action.mail.send").unwrap().requires_approval());
        assert!(facts("action.set.order.state").unwrap().requires_approval());
        assert!(!facts("action.add.order.tag").unwrap().requires_approval());
        assert!(facts("action.run.sql").is_none());
        assert_eq!(permissions("action.run.sql"), ["orders.write"]);
        assert_eq!(permissions("ai_proposal")[0], "catalog.write");
        use veyndo_flow_graph::catalog::{FLOW_ACTIONS, PIPELINE_ACTIONS};
        assert!(super::covers(ACTIONS, PIPELINE_ACTIONS, FLOW_ACTIONS));
        assert!(!super::covers(
            &ACTIONS[1..],
            PIPELINE_ACTIONS,
            FLOW_ACTIONS
        ));
        assert!(!super::covers(
            &ACTIONS[..ACTIONS.len() - 1],
            PIPELINE_ACTIONS,
            FLOW_ACTIONS
        ));
        let mut swapped = ACTIONS.to_vec();
        swapped.swap(4, 5);
        assert!(!super::covers(&swapped, PIPELINE_ACTIONS, FLOW_ACTIONS));
        let mut unknown = ACTIONS.to_vec();
        unknown.push(("action.run.sql", ACTIONS[0].1));
        assert!(!super::covers(&unknown, PIPELINE_ACTIONS, FLOW_ACTIONS));
    }

    #[test]
    fn each_action_names_the_right_it_needs() {
        assert_eq!(permission("action.add.customer.tag"), "customers.write");
        assert_eq!(
            permission("action.change.customer.group"),
            "customers.write"
        );
        assert_eq!(permission("action.generate.document"), "documents.create");
        assert_eq!(permission("action.mail.send"), "apps.manage");
        assert_eq!(permission("app_action"), "apps.manage");
        assert_eq!(permission("ai_proposal"), "catalog.write");
        assert_eq!(permission("knowledge.extract"), "knowledge.write");
        assert_eq!(permission("note"), "automation.manage");
        assert_eq!(permission("action.stop.flow"), "automation.manage");
        assert_eq!(permission("action.set.order.state"), "orders.write");
    }

    #[test]
    fn source_names_become_native_ones_without_replacing_native_values() {
        let c = normalize(
            "action.add.order.tag",
            &json!({"tagIds":["a"],"customerGroupId":"vip","documentType":"invoice"}),
        );
        assert_eq!(c["tags"], json!(["a"]));
        assert_eq!(c["groupId"], "vip");
        assert_eq!(c["kind"], "invoice");
        let kept = normalize(
            "action.add.order.tag",
            &json!({"tagIds":["a"],"tags":["b"]}),
        );
        assert_eq!(kept["tags"], json!(["b"]));
        assert_eq!(
            normalize("action.grant.download.access", &json!({}))["value"],
            true
        );
        assert_eq!(
            normalize("action.grant.download.access", &json!({"value":false}))["value"],
            false
        );
        assert!(normalize("note", &json!({})).get("value").is_none());
    }

    #[test]
    fn actions_accept_only_their_complete_configuration() {
        for (action, config) in [
            ("note", json!({"instruction":{"en-GB":"Check the order"}})),
            ("knowledge.extract", json!({"sourceId":"s1"})),
            ("action.add.order.tag", json!({"tagIds":["vip"]})),
            (
                "action.set.order.custom.field",
                json!({"field":"priority","value":1}),
            ),
            (
                "action.change.customer.group",
                json!({"customerGroupId":"business"}),
            ),
            ("action.change.customer.status", json!({"active":false})),
            ("action.grant.download.access", json!({})),
            (
                "action.set.order.state",
                json!({"kind":"payment","state":"paid"}),
            ),
            (
                "action.generate.document",
                json!({"documentType":"invoice"}),
            ),
            (
                "app_action",
                json!({"app":"email","action":"send","arguments":{}}),
            ),
            (
                "action.mail.send",
                json!({"templateId":"order_confirmation"}),
            ),
            (
                "action.add.order.affiliate.and.campaign.code",
                json!({"affiliateCode":"a","campaignCode":"c"}),
            ),
            ("action.stop.flow", json!({})),
        ] {
            assert!(validate(action, &config).is_ok(), "{action}");
        }
        for (action, config) in [
            ("action.run.sql", json!({})),
            ("note", json!([])),
            ("note", json!({})),
            ("note", json!({"instruction":{"en-GB":" "}})),
            ("knowledge.extract", json!({"query":"x"})),
            ("action.add.order.tag", json!({"tags":[]})),
            ("action.add.order.tag", json!({"tags":[""]})),
            (
                "action.set.order.custom.field",
                json!({"field":"a.b","value":1}),
            ),
            ("action.set.order.custom.field", json!({"field":"priority"})),
            (
                "action.change.customer.group",
                json!({"groupId":"Not valid"}),
            ),
            ("action.change.customer.status", json!({"active":"no"})),
            ("action.grant.download.access", json!({"value":"yes"})),
            ("action.set.order.state", json!({"kind":"payment"})),
            ("action.generate.document", json!({"kind":"credit_note"})),
            ("app_action", json!({"app":"email","action":"send"})),
            ("action.mail.send", json!({"templateId":"newsletter"})),
            (
                "action.add.order.affiliate.and.campaign.code",
                json!({"affiliateCode":"a"}),
            ),
            ("action.stop.flow", json!({"note":"x".repeat(8000)})),
        ] {
            assert!(validate(action, &config).is_err(), "{action} {config}");
        }
    }
}
