//! Contract of `veyndo_payments::ports::Host::service`, and a test adapter with a fixed service map.
use serde_json::Value;
use std::collections::HashMap;
use veyndo_core::{PortFuture, Result, bad};
use veyndo_payments::ports::Host;

/// A provider version nobody configured is refused, and a configured one (when the caller
/// names it) answers with the same service in every call.
pub fn service(host: &dyn Host, configured: Option<(&str, &str, &str)>) {
    for (provider, version, environment) in [
        ("no-such-provider", "1.0.0", "sandbox"),
        ("no-such-provider", "1.0.0", "live"),
        ("", "", ""),
    ] {
        assert!(
            host.service(provider, version, environment).is_err(),
            "payments Host: unconfigured service {provider} {version} {environment} answered"
        );
    }
    if let Some((provider, version, environment)) = configured {
        let first = host.service(provider, version, environment);
        assert!(first.is_ok(), "payments Host: configured service refused");
        assert_eq!(
            first.ok(),
            host.service(provider, version, environment).ok(),
            "payments Host: the same service differs between calls"
        );
    }
}

/// The test adapter: services by provider, version and environment; captures change nothing.
#[derive(Default)]
pub struct Services {
    pub services: HashMap<(String, String, String), Value>,
}

impl Host for Services {
    fn service(&self, provider: &str, version: &str, environment: &str) -> Result<Value> {
        self.services
            .get(&(provider.into(), version.into(), environment.into()))
            .cloned()
            .ok_or(bad("Payment service not configured"))
    }
    fn captured<'a, 'c: 'a>(
        &'a self,
        _tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _tenant: &'a str,
        _order: &'a mut Value,
        _state: &'a str,
    ) -> PortFuture<'a, ()> {
        crate::admitted()
    }
}

#[cfg(test)]
mod tests {
    use super::{Services, service};
    use serde_json::json;

    #[test]
    fn the_test_adapter_meets_the_contract() {
        let mut adapter = Services::default();
        adapter.services.insert(
            ("fixture".into(), "1.0.0".into(), "sandbox".into()),
            json!({"url":"https://payments.example.test"}),
        );
        service(&adapter, Some(("fixture", "1.0.0", "sandbox")));
    }
}
