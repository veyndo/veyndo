//! Test adapter for the payments port `Gateway`, so the payments mirror builds and tests without the server.
use serde_json::{Value, json};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{PortFuture, Result, bad};
use veyndo_payments::domain::attempt::Attempt;
use veyndo_payments::ports::Gateway;

/// Polls a test adapter's answer; adapters answer at once, without a runtime.
fn ready<T>(mut future: PortFuture<'_, T>) -> Result<T> {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a test adapter answers at once"),
    }
}

/// A provider fixture: calls echo their input, operations succeed, no package is installed.
struct FixtureProvider;

impl Gateway for FixtureProvider {
    fn call<'a>(
        &'a self,
        _: &'a str,
        provider: &'a str,
        _: &'a str,
        environment: &'a str,
        path: &'a str,
        body: &'a Value,
    ) -> PortFuture<'a, Value> {
        let answer =
            json!({"provider": provider, "environment": environment, "path": path, "body": body});
        Box::pin(async move { Ok(answer) })
    }
    fn dispatch<'a>(
        &'a self,
        attempt: &'a Attempt,
        operation: &'a str,
        key: &'a str,
        _: &'a Value,
    ) -> PortFuture<'a, Value> {
        let answer = json!({"attempt": attempt.id, "operation": operation, "key": key, "status": "succeeded"});
        Box::pin(async move { Ok(answer) })
    }
    fn persist<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a Attempt,
        _: &'a str,
        _: &'a Value,
        _: &'a Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn scope(&self) -> (Vec<String>, Vec<String>) {
        (vec!["shop-a".into()], vec!["fixture".into()])
    }
    fn cancellable(&self, _: &Attempt, evidence: &Value) -> Result<()> {
        if evidence["status"] == "captured" {
            Err(bad("a captured payment is refunded, not cancelled"))
        } else {
            Ok(())
        }
    }
    fn package<'a>(&'a self, _: &'a str, id: &'a str) -> PortFuture<'a, Manifest> {
        let message = format!("package {id} is not installed");
        Box::pin(async move { Err(bad(message)) })
    }
    fn builtin(&self, tenant: &str) -> Value {
        json!({"tenant": tenant, "environment": "sandbox", "configured": false})
    }
}

#[test]
fn gateway_port_answers_without_the_server() {
    let gateway: Arc<dyn Gateway> = Arc::new(FixtureProvider);
    let body = json!({"amount": 100});
    let answer =
        ready(gateway.call("shop-a", "fixture", "1", "sandbox", "/orders", &body)).unwrap();
    assert_eq!(answer["body"]["amount"], 100);
    assert_eq!(gateway.scope().1, vec!["fixture".to_string()]);
    assert!(ready(gateway.package("shop-a", "unknown")).is_err());
    assert_eq!(gateway.builtin("shop-a")["configured"], false);
}

/// A shop without payment history and with one sales channel, `storefront`.
impl veyndo_payments::ports::MerchantRecords for FixtureProvider {
    fn payment_history<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
        _: &'a str,
    ) -> PortFuture<'a, Vec<veyndo_payments::ports::PaymentChange>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn channel_exists<'a>(&'a self, _: &'a str, channel: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async move { Ok(channel == "storefront") })
    }
    fn channel_ids<'a>(&'a self, _: &'a str) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { Ok(vec!["storefront".to_string()]) })
    }
}
