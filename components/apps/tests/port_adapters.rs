//! Test adapters for the app platform ports (`RecordRules`, `CommerceInstall`, `HostActions`, `HostedInstall`), so the apps mirror builds and tests without the server.
use serde_json::{Value, json};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_platform::ports::{
    ActionFuture, CommerceInstall, EnvironmentFuture, HostActions, HostCall, HostedFuture,
    HostedInstall, RecordRules, RuleFuture,
};
use veyndo_core::{PortFuture, RequestContext, Result, bad};

type Tx<'a, 'c> = &'a mut sqlx::Transaction<'c, sqlx::Postgres>;

/// Polls a test adapter's answer; adapters answer at once, without a runtime.
fn ready<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a test adapter answers at once"),
    }
}

fn manifest() -> Manifest {
    serde_json::from_str(include_str!(
        "../../../extensions/apps/assistant-examples/frontend.json"
    ))
    .unwrap()
}

/// Bundled apps without record rules: records must be objects and pass unchanged.
struct ObjectRecords;

impl RecordRules for ObjectRecords {
    fn normalize(&self, _: &Manifest, v: &Value) -> Result<Value> {
        if v.is_object() {
            Ok(v.clone())
        } else {
            Err(bad("a record is an object"))
        }
    }
    fn check(&self, _: &Manifest, _: &Entity, fields: &Value) -> Result<()> {
        self.normalize(&manifest(), fields).map(|_| ())
    }
    fn planning_format(&self, m: &Manifest) -> Value {
        json!({"app": m.id, "format": "plain"})
    }
    fn validate<'a, 'c: 'a>(
        &'a self,
        _: Tx<'a, 'c>,
        _: &'a str,
        _: &'a Manifest,
        _: &'a Entity,
        _: &'a Value,
    ) -> RuleFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}

/// An installation without commerce side effects.
struct NoCommerce;

impl CommerceInstall for NoCommerce {
    fn lock<'a, 'c: 'a>(&'a self, _: Tx<'a, 'c>, _: &'a str) -> RuleFuture<'a> {
        Box::pin(async { Ok(()) })
    }
    fn install<'a, 'c: 'a>(&'a self, _: Tx<'a, 'c>, _: &'a str, _: &'a Manifest) -> RuleFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}

/// No server-owned actions; records each purge so a test can see it happened.
#[derive(Default)]
struct RecordedActions(Mutex<Vec<String>>);

impl HostActions for RecordedActions {
    fn hosted<'a>(&'a self, _: &'a str, _: &'a str, _: &'a str) -> HostedFuture<'a> {
        Box::pin(async { Ok(None) })
    }
    fn run<'a>(&'a self, call: HostCall<'a>) -> ActionFuture<'a> {
        let answer = json!({"tenant": call.tenant, "input": call.input});
        Box::pin(async move { Ok(answer) })
    }
    fn visitor_object<'a>(
        &'a self,
        _: &'a RequestContext,
        _: &'a str,
        _: &'a str,
    ) -> RuleFuture<'a> {
        Box::pin(async { Err(bad("no visitor objects in this test")) })
    }
    fn purge_sources<'a>(&'a self, tenant: &'a str, app: &'a str, cursor: i64) -> RuleFuture<'a> {
        self.0
            .lock()
            .unwrap()
            .push(format!("{tenant}/{app}@{cursor}"));
        Box::pin(async { Ok(()) })
    }
    fn create_preview<'a>(
        &'a self,
        _: &'a RequestContext,
        actor: &'a str,
    ) -> EnvironmentFuture<'a> {
        let preview = format!("preview-{actor}");
        Box::pin(async move { Ok(preview) })
    }
}

/// Hosted packages are already in place.
struct Installed;

impl HostedInstall for Installed {
    fn ensure<'a, 'c: 'a>(&'a self, _: Tx<'a, 'c>, _: &'a str, _: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn app_platform_ports_answer_without_the_server() {
    let rules: &'static dyn RecordRules = &ObjectRecords;
    let m = manifest();
    assert!(rules.normalize(&m, &json!({"title": "x"})).is_ok());
    assert!(rules.normalize(&m, &json!("x")).is_err());
    assert_eq!(rules.planning_format(&m)["format"], "plain");
    let _commerce: &'static dyn CommerceInstall = &NoCommerce;
    let _hosted: Arc<dyn HostedInstall> = Arc::new(Installed);
    let recorded = Arc::new(RecordedActions::default());
    let actions: Arc<dyn HostActions> = recorded.clone();
    let request = RequestContext::new();
    assert!(
        ready(actions.hosted("shop-a", "app", "sync"))
            .unwrap()
            .is_none()
    );
    ready(actions.purge_sources("shop-a", "app", 7)).unwrap();
    assert_eq!(recorded.0.lock().unwrap().as_slice(), ["shop-a/app@7"]);
    assert_eq!(
        ready(actions.create_preview(&request, "u-1")).unwrap(),
        "preview-u-1"
    );
    assert!(ready(actions.visitor_object(&request, "k", "1")).is_err());
}
