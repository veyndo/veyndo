//! Test adapters for the marketing ports (`FlowEffects`, `ConfigurationHost`), so the marketing mirror builds and tests without the server.
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use veyndo_core::{PortFuture, RequestContext, State};
use veyndo_marketing::ports::{ConfigurationHost, FlowEffects};
use veyndo_marketing::types::flow::Flow;

/// Runs each action once per idempotency key and returns the recorded result on a repeat.
#[derive(Default)]
struct RecordedEffects(Mutex<HashMap<String, Value>>);

impl RecordedEffects {
    fn run(&self, key: &str, action: &str, config: &Value) -> Value {
        self.0
            .lock()
            .unwrap()
            .entry(key.to_string())
            .or_insert_with(|| json!({"action": action, "config": config}))
            .clone()
    }
}

impl FlowEffects for RecordedEffects {
    fn execute<'a>(
        &'a self,
        _: &'a str,
        _: &'a Flow,
        key: &'a str,
        action: &'a str,
        config: &'a Value,
        _: &'a Value,
    ) -> PortFuture<'a, Value> {
        let result = self.run(key, action, config);
        Box::pin(async move { Ok(result) })
    }
}

/// Admits every template, app step and channel; holds the shared state of the test.
struct AdmitAll(State);

impl ConfigurationHost for AdmitAll {
    fn core(&self) -> &State {
        &self.0
    }
    fn validate_template<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a str,
        _: &'a RequestContext,
        _: &'a Flow,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn validate_apps<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a str,
        _: &'a RequestContext,
        _: &'a Flow,
        _: bool,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn configure_channel<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a str,
        _: &'a str,
        _: &'a Value,
        _: bool,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn flow_effects_port_runs_an_action_once_per_key() {
    let effects = Arc::new(RecordedEffects::default());
    let _port: Arc<dyn FlowEffects> = effects.clone();
    let first = effects.run("key-1", "tag", &json!({"tag": "vip"}));
    let repeat = effects.run("key-1", "tag", &json!({"tag": "other"}));
    assert_eq!(first, repeat, "a repeated key returns the recorded result");
}

/// A database state needs a runtime this crate does not depend on; the adapter is checked against the port here.
#[test]
fn configuration_host_port_is_satisfiable_without_the_server() {
    fn host(state: State) -> Arc<dyn ConfigurationHost> {
        Arc::new(AdmitAll(state))
    }
    let _ = host as fn(State) -> Arc<dyn ConfigurationHost>;
}
