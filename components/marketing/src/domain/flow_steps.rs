//! The app steps of a flow: a simple app action and every app or mail node of its pipeline, seen as one app flow each.
use crate::domain::pipeline::Node;
use crate::types::flow::{AppFlowAction, Flow};
use serde_json::json;

/// Configuration save, release and app replacement inspect the same app steps of a flow.
pub fn app_steps(f: &Flow) -> Vec<Flow> {
    let mut steps = vec![];
    if f.action == "app_action" {
        steps.push(f.clone());
    }
    if let Some(p) = &f.pipeline {
        for node in &p.nodes {
            if let Node::Action { action, config, .. } = node
                && (action == "app_action" || action == "action.mail.send")
            {
                let mut step = f.clone();
                step.action = "app_action".into();
                step.app_action = Some(AppFlowAction {
                    app: config["app"].as_str().unwrap_or("email").into(),
                    action: config["action"].as_str().unwrap_or("send_order").into(),
                    arguments: if action == "action.mail.send" {
                        json!({"locale":&f.locale[..2],"dryRun":false})
                    } else {
                        config["arguments"].clone()
                    },
                });
                steps.push(step);
            }
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn flow(action: &str, pipeline: Value) -> Flow {
        serde_json::from_value(json!({
            "pipeline": pipeline,
            "name": {"en-GB": "Flow"},
            "active": true,
            "event": "order.placed",
            "condition": {"type": "andContainer", "children": []},
            "action": action,
            "instruction": {},
            "locale": "de-DE",
            "appAction": if action == "app_action" {
                json!({"app": "crm", "action": "sync", "arguments": {}})
            } else {
                Value::Null
            },
        }))
        .unwrap()
    }

    #[test]
    fn app_steps_cover_simple_actions_and_mail_nodes() {
        assert_eq!(app_steps(&flow("app_action", Value::Null)).len(), 1);
        assert!(app_steps(&flow("ai_proposal", Value::Null)).is_empty());
        let mail = flow(
            "ai_proposal",
            json!({"entry": "m", "nodes": [{"kind": "action", "id": "m", "action": "action.mail.send", "config": {}, "next": null}]}),
        );
        let steps = app_steps(&mail);
        assert_eq!(steps.len(), 1);
        let target = steps[0].app_action.as_ref().unwrap();
        assert_eq!(
            (target.app.as_str(), target.action.as_str()),
            ("email", "send_order")
        );
        assert_eq!(target.arguments["locale"], "de");
    }
}
