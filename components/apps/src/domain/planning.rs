//! Merchant planning over app records: what a proposal may bind and how much untrusted context a model sees.
use crate::domain::action_admission;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_app_model::Action;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{RequestContext, Result, bad};
use veyndo_identity::permit;

#[derive(Clone, Debug, Serialize, Deserialize)]
/// One proposed managed save of an app record, bound to the package revision it was planned against.
pub struct AppChange {
    pub app: String,
    pub action: String,
    pub arguments_json: String,
    #[serde(default)]
    pub expected_app_revision: i64,
}
/// App descriptions/records are untrusted data; bound nested JSON before entering a model prompt.
pub fn bounded_context(v: &Value, depth: usize) -> Value {
    if depth >= 4 {
        return json!("[context depth limit]");
    }
    match v {
        Value::String(s) => json!(s.chars().take(300).collect::<String>()),
        Value::Array(items) => json!(
            items
                .iter()
                .take(8)
                .map(|v| bounded_context(v, depth + 1))
                .collect::<Vec<_>>()
        ),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .take(16)
                .map(|(k, v)| (k.clone(), bounded_context(v, depth + 1)))
                .collect(),
        ),
        _ => v.clone(),
    }
}

/// A proposal is untrusted model output; both selection and current action rights are checked before binding and committing.
pub fn authorize_planned_action(m: &Manifest, action: &Action, h: &RequestContext) -> Result<()> {
    if action.mcp == Some(false)
        || m.intelligence
            .as_ref()
            .is_some_and(|ai| !ai.tools.contains(&action.name))
    {
        return Err(bad("App action is not selected for merchant planning"));
    }
    action_admission::additional(h, action)?;
    permit(h, action.permission.as_deref().unwrap_or("catalog"))
}

#[cfg(test)]
mod tests {
    use super::{authorize_planned_action, bounded_context};
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::RequestContext;

    fn manifest() -> Manifest {
        serde_json::from_str(include_str!(
            "../../../../extensions/apps/assistant-examples/mcp-app.json"
        ))
        .unwrap()
    }

    #[test]
    fn untrusted_context_is_bounded_in_depth_width_and_length() {
        let long = "x".repeat(400);
        let wide: Vec<_> = (0..20).collect();
        let bounded = bounded_context(&json!({"text": long, "list": wide, "n": 7}), 0);
        assert_eq!(bounded["text"].as_str().unwrap().len(), 300);
        assert_eq!(bounded["list"].as_array().unwrap().len(), 8);
        assert_eq!(bounded["n"], 7);
        let deep = json!({"a":{"b":{"c":{"d":{"e":1}}}}});
        assert_eq!(
            bounded_context(&deep, 0)["a"]["b"]["c"]["d"],
            "[context depth limit]"
        );
        let many: serde_json::Map<_, _> = (0..30).map(|i| (format!("k{i:02}"), json!(i))).collect();
        assert_eq!(
            bounded_context(&json!(many), 0).as_object().unwrap().len(),
            16
        );
    }

    #[test]
    fn planning_binds_only_selected_actions_the_actor_may_run() {
        let mut m = manifest();
        let mut editor = RequestContext::new();
        editor.insert("x-rac-role", "editor".parse().unwrap());
        let viewer = {
            let mut h = RequestContext::new();
            h.insert("x-rac-role", "viewer".parse().unwrap());
            h
        };
        // The fixture's save action opted out of MCP and is therefore not offered to planning.
        assert!(authorize_planned_action(&m, &m.actions[1], &editor).is_err());
        m.actions[1].mcp = None;
        // The package selects its planning tools explicitly; an empty selection offers none.
        assert!(authorize_planned_action(&m, &m.actions[1], &editor).is_err());
        let name = m.actions[1].name.clone();
        m.intelligence.as_mut().unwrap().tools.push(name);
        assert!(authorize_planned_action(&m, &m.actions[1], &editor).is_ok());
        assert!(authorize_planned_action(&m, &m.actions[1], &viewer).is_err());
    }
}
