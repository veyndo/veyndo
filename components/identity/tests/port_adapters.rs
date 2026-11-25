//! Test adapter for the identity port `RoleLanguages`, so the identity mirror builds and tests without the server.
use serde_json::Value;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use veyndo_core::{PortFuture, Result, bad};
use veyndo_identity::ports::RoleLanguages;

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

/// A shop with German as main language and English enabled.
struct FixedLanguages;

impl RoleLanguages for FixedLanguages {
    fn languages<'a>(&'a self, _: &'a str) -> PortFuture<'a, (String, Vec<String>)> {
        Box::pin(async { Ok(("de-DE".into(), vec!["de-DE".into(), "en-GB".into()])) })
    }
    fn check_names<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a str,
        names: &'a Value,
    ) -> PortFuture<'a, ()> {
        let answer = if names.get("de-DE").is_some() {
            Ok(())
        } else {
            Err(bad("main language name missing"))
        };
        Box::pin(async move { answer })
    }
}

#[test]
fn role_languages_port_answers_main_and_enabled_languages() {
    let port: Arc<dyn RoleLanguages> = Arc::new(FixedLanguages);
    let (main, enabled) = ready(port.languages("shop-a")).unwrap();
    assert_eq!(main, "de-DE");
    assert!(enabled.contains(&main));
}
