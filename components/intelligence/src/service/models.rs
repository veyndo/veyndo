//! The model context of intelligence operations: shared state, the configured inference providers and this process's inference slots.
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use veyndo_cognition::inference::{Choice, Inference};
use veyndo_core::{Error, Result, State, StatusCode, bad};

/// What a model call needs; the server lends its own state, providers and slots.
#[derive(Clone, Copy)]
pub struct Models<'a> {
    /// Database pool and HTTP client.
    pub core: &'a State,
    /// The configured providers with their admission.
    pub inference: &'a Inference,
    /// Process-wide inference concurrency.
    pub slots: &'a Arc<Semaphore>,
}
impl Models<'_> {
    /// A free inference slot of this process, or a busy refusal.
    pub fn slot(&self) -> Result<OwnedSemaphorePermit> {
        self.slots.clone().try_acquire_owned().map_err(|_| {
            Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Inference capacity busy".into(),
            )
        })
    }
}
/// The model state the server owns for its lifetime: configured providers, inference slots and the default model name.
#[derive(Clone)]
pub struct Inferencing {
    /// The configured providers with their admission.
    pub inference: Inference,
    /// Process-wide inference concurrency.
    pub slots: Arc<Semaphore>,
    /// The default local model name shown in health and provider views.
    pub model: Arc<String>,
}
impl Inferencing {
    /// The borrowed model context of one operation.
    pub fn models<'a>(&'a self, core: &'a State) -> Models<'a> {
        Models {
            core,
            inference: &self.inference,
            slots: &self.slots,
        }
    }
}
/// The inference selection of a request; absent or null means the shop default.
pub fn choice(v: &Value) -> Result<Option<Choice>> {
    v.get("inference")
        .filter(|v| !v.is_null())
        .map(|v| serde_json::from_value(v.clone()).map_err(|_| bad("Invalid inference selection")))
        .transpose()
}
/// The configured embedding model of this installation.
pub fn embedding_model() -> String {
    std::env::var("EMBEDDING_MODEL").unwrap_or("qwen3-embedding:0.6b".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn choice_is_optional_and_typed() {
        assert!(choice(&json!({})).unwrap().is_none());
        assert!(choice(&json!({"inference":null})).unwrap().is_none());
        assert!(
            choice(&json!({"inference":{"provider":"ollama"}}))
                .unwrap()
                .is_some()
        );
        assert!(choice(&json!({"inference":{"provider":"unknown"}})).is_err());
    }
    #[test]
    fn embedding_model_has_a_default() {
        assert!(!embedding_model().is_empty());
    }
}
