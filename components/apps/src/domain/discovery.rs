//! Search over installed app capabilities with current rights: operations, their schemas and relevance.
use crate::domain::action_admission;
use serde_json::{Value, json};
use veyndo_app_model::Action;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::RequestContext;

/// The discovery operations agents may call.
pub const CAPABILITIES: &[(&str, &str)] = &[
    (
        "merchant.apps.search",
        "Find relevant installed apps and their admitted read tools by query; follow nextAfter",
    ),
    (
        "merchant.apps.read",
        "Read an explicitly discovered managed app model through its current declared read action",
    ),
];
/// The input schema of a discovery operation.
pub fn schema(name: &str) -> Option<Value> {
    let (properties, required) = match name {
        "merchant.apps.search" => (
            json!({"query":{"type":"string","maxLength":4000},"after":{"type":"string","maxLength":160},"limit":{"type":"integer","minimum":1,"maximum":20}}),
            vec![],
        ),
        "merchant.apps.read" => (
            json!({"app":{"type":"string","maxLength":160},"action":{"type":"string","maxLength":160},"arguments":{"type":"object"}}),
            vec!["app", "action", "arguments"],
        ),
        _ => return None,
    };
    Some(
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
    )
}
/// How many words of a query an app and its actions mention.
pub fn relevant(m: &Manifest, query: &str) -> usize {
    let text = json!([
        m.id,
        m.name,
        m.intelligence.as_ref().map(|i| &i.description),
        m.actions
            .iter()
            .map(|a| (&a.name, &a.description))
            .collect::<Vec<_>>()
    ])
    .to_string()
    .to_lowercase();
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.chars().count() > 2)
        .map(str::to_lowercase)
        .filter(|s| text.contains(s))
        .count()
}
/// Whether an action is offered to agents: not withheld from MCP, authorized now and listed as a tool.
pub fn admitted(h: &RequestContext, m: &Manifest, action: &Action) -> bool {
    action.mcp != Some(false)
        && action_admission::authorized(h, action)
        && m.intelligence
            .as_ref()
            .is_none_or(|i| i.tools.contains(&action.name))
}

#[cfg(test)]
mod tests {
    use super::{CAPABILITIES, admitted, relevant, schema};
    use crate::domain::approval::built_in;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::RequestContext;

    #[test]
    fn every_operation_has_a_schema_and_relevance_counts_real_words() {
        for (name, _) in CAPABILITIES {
            assert_eq!(schema(name).unwrap()["additionalProperties"], false);
        }
        assert!(schema("merchant.apps.unknown").is_none());
        let m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        assert!(relevant(&m, "engraving") >= 1);
        assert_eq!(relevant(&m, "zz qq"), 0);
        assert_eq!(relevant(&m, "xylophone-quartz"), 0);
    }

    #[test]
    fn an_action_withheld_from_agents_is_never_admitted() {
        let m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        let mut action = m.actions[0].clone();
        action.public = true;
        action.mcp = Some(false);
        assert!(!admitted(&RequestContext::new(), &m, &action));
    }
}
