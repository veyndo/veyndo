//! Native action schema and permissions use original Core names; no arbitrary SQL, shell or unguarded payment transitions.
use crate::domain::flow_text;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_core::document::view;
use veyndo_core::{Result, bad};
use veyndo_flow_graph::catalog::PIPELINE_ACTIONS;
pub fn permission(action: &str) -> &'static str {
    if action.contains("customer") {
        "customers.write"
    } else {
        match action {
            "action.generate.document" => "documents.create",
            "action.mail.send" | "app_action" => "apps.manage",
            "ai_proposal" | "work_proposal" => "catalog.write",
            "knowledge.extract" => "knowledge.write",
            "note" | "action.stop.flow" => "automation.manage",
            _ => "orders.write",
        }
    }
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
    if action == "action.grant.download.access" && c.get("value").is_none() {
        c["value"] = json!(true);
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
    use super::{normalize, permission, validate};
    use serde_json::json;

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
