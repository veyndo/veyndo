//! The flow record: event, condition, action and its localized instruction, validated before it is stored.
use crate::domain::{flow_text, pipeline::Pipeline, rules};
use crate::types::work_template::WorkTemplate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_app_model::declarations::identifier;
use veyndo_cognition::inference::Choice;
use veyndo_core::localized::valid_locale_key;
use veyndo_core::{Result, bad};
use veyndo_flow_graph::catalog::{EVENTS, FLOW_ACTIONS};
pub fn knowledge_event(kind: &str) -> bool {
    EVENTS.contains(&kind)
        && (kind.starts_with("knowledge.")
            || kind.starts_with("intelligence.")
            || kind == "merchant.change.applied"
            || kind == "app.configuration.changed"
            || kind.starts_with("consumer.")
            || kind.starts_with("privacy.")
            || kind.starts_with("b2b.")
            || kind.starts_with("pos."))
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Flow {
    #[serde(default)]
    pub pipeline: Option<Pipeline>,
    pub name: HashMap<String, String>,
    pub active: bool,
    pub event: String,
    pub condition: rules::Condition,
    pub action: String,
    pub instruction: HashMap<String, Option<String>>,
    pub locale: String,
    #[serde(default)]
    pub inference: Option<Choice>,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub app_action: Option<AppFlowAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_template: Option<WorkTemplate>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppFlowAction {
    pub app: String,
    pub action: String,
    #[serde(default)]
    pub arguments: Value,
}
impl Flow {
    pub fn validate(&self) -> Result<()> {
        self.condition.validate(0)?;
        if self.action == "work_proposal" {
            self.work_template
                .as_ref()
                .ok_or(bad("Work template required"))?
                .validate()?;
        } else if self.work_template.is_some() {
            return Err(bad("Work template requires its review action"));
        }
        if self.action == "pipeline" {
            self.pipeline
                .as_ref()
                .ok_or(bad("Flow pipeline required"))?
                .validate()?;
        }
        if self.action == "app_action"
            && self.app_action.as_ref().is_none_or(|x| {
                !identifier(&x.app)
                    || !identifier(&x.action)
                    || !x.arguments.is_object()
                    || x.arguments.to_string().len() > 8000
            })
        {
            return Err(bad("Invalid flow app action"));
        }
        if !EVENTS.contains(&self.event.as_str()) && !valid_app_event(&self.event)
            || !FLOW_ACTIONS.contains(&self.action.as_str())
            || !valid_locale_key(&self.locale)
        {
            return Err(bad("Unsupported flow action, event or locale"));
        }
        if self.action != "pipeline" {
            flow_text::shape(&json!(self.instruction))?;
        }
        Ok(())
    }
}
/// Only namespaced app events can extend the standard event catalogue.
pub fn valid_app_event(name: &str) -> bool {
    let parts = name.split('.').collect::<Vec<_>>();
    parts.len() == 3 && parts[0] == "app" && identifier(parts[1]) && identifier(parts[2])
}
