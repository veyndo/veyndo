//! Bounded copies of reviewed native work: event values replace explicit scalar fields, never executable text.
use serde::{Deserialize, Serialize};
use serde_json::json;
use veyndo_app_model::declarations::identifier;
use veyndo_core::{Result, bad};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkTemplate {
    pub task_id: String,
    #[serde(default)]
    pub bindings: Vec<Binding>,
    pub daily_limit: u32,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub step: usize,
    pub pointer: String,
    pub source: String,
}

impl WorkTemplate {
    pub fn validate(&self) -> Result<()> {
        if self.task_id.is_empty()
            || !self
                .task_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
            || self.task_id.len() > 64
            || json!(self).to_string().len() > 16384
            || self.bindings.len() > 48
            || !(1..=100).contains(&self.daily_limit)
        {
            return Err(bad(
                "Work template needs a saved task, at most 48 bindings, 16 KiB and a daily limit of 1..100",
            ));
        }
        let mut targets = std::collections::HashSet::new();
        for binding in &self.bindings {
            let source = binding.source.as_str();
            let generated = source.strip_prefix("run.key:").is_some_and(identifier);
            if !generated
                && ![
                    "event.productId",
                    "event.revision",
                    "event.orderId",
                    "event.documentId",
                ]
                .contains(&source)
            {
                return Err(bad("Unsupported work template source"));
            }
            if binding.pointer.is_empty()
                || binding.pointer.len() > 300
                || !targets.insert((binding.step, &binding.pointer))
                || binding.step >= 12
            {
                return Err(bad(
                    "Map each work template input to one existing text or number field",
                ));
            }
        }
        Ok(())
    }
}
