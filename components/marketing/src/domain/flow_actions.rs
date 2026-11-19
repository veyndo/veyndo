//! Native action schema and permissions use original Core names; no arbitrary SQL, shell or unguarded payment transitions.
use crate::domain::flow_text;
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
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
    if !PIPELINE_ACTIONS.contains(&action) || !config.is_object() || config.to_string().len() > 8000
    {
        return Err(bad("Unknown flow action or invalid configuration"));
    }
    if matches!(action, "note" | "ai_proposal") {
        flow_text::shape(&config["instruction"])?;
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
        && config["tags"].as_array().is_none_or(|a| {
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
        if config["field"]
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
        if !identifier(config["groupId"].as_str().unwrap()) {
            return Err(bad("Unknown native customer group"));
        }
    }
    if action == "action.change.customer.status" && !config["active"].is_boolean() {
        return Err(bad("Customer active flag required"));
    }
    if action == "action.grant.download.access" && !config["value"].is_boolean() {
        return Err(bad("Download access boolean required"));
    }
    if action == "action.set.order.state" {
        require("kind")?;
        require("state")?;
    }
    if action == "action.generate.document" {
        require("kind")?;
        if !["invoice", "delivery_note"].contains(&config["kind"].as_str().unwrap()) {
            return Err(bad("Unsupported automatic document kind"));
        }
    }
    if action == "app_action" {
        require("app")?;
        require("action")?;
        if !config["arguments"].is_object() {
            return Err(bad("App arguments object required"));
        }
    }
    if action == "action.mail.send" {
        require("templateId")?;
        if config["templateId"] != "order_confirmation" {
            return Err(bad("Unknown installed email template"));
        }
    }
    if action.ends_with("affiliate.and.campaign.code") {
        require("affiliateCode")?;
        require("campaignCode")?;
    }
    Ok(())
}
