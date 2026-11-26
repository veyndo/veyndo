//! Pure startup validation of the native provider origin; no live credentials or provider calls.
use crate::domain::service_config::validate_service;
use serde::Deserialize;
use serde_json::Value;
use std::env;
use veyndo_core::document::view;

/// The partner attribution code of a configured PayPal account.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Attribution {
    bn_code: Value,
}

/// The address and environment of one configured adapter service version.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ServiceEntry {
    url: Value,
    environment: Value,
}
/// The provider origin for the configured environment; anything else stops the start.
pub fn paypal_base() -> String {
    let live = env::var("PAYPAL_ENVIRONMENT").unwrap_or("sandbox".into());
    if !["live", "sandbox"].contains(&live.as_str()) {
        panic!("PAYPAL_ENVIRONMENT must be live or sandbox");
    }
    let expected = if live == "live" {
        "https://api-m.paypal.com"
    } else {
        "https://api-m.sandbox.paypal.com"
    };
    let base = env::var("PAYPAL_SANDBOX_BASE_URL").unwrap_or(expected.into());
    let url = reqwest::Url::parse(&base).expect("Invalid PayPal URL");
    if base != expected
        && !(live == "sandbox"
            && url.scheme() == "http"
            && ["127.0.0.1", "localhost"].contains(&url.host_str().unwrap_or(""))
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none())
    {
        panic!("PayPal origin must match its configured environment");
    }
    base.trim_end_matches('/').into()
}
/// Which provider environment this process talks to.
pub fn paypal_environment() -> &'static str {
    if env::var("PAYPAL_SANDBOX_BASE_URL").is_ok_and(|s| s.starts_with("http://")) {
        "contract-fixture"
    } else if env::var("PAYPAL_ENVIRONMENT").is_ok_and(|s| s == "live") {
        "live"
    } else {
        "sandbox"
    }
}

/// Validate every declared provider at startup, without embedding credential values in diagnostics.
pub fn validate_accounts(accounts: &serde_json::Value, services: &serde_json::Value) {
    for account in accounts
        .as_object()
        .expect("PayPal accounts object")
        .values()
    {
        for key in ["clientId", "clientSecret", "bnCode"] {
            assert!(
                account[key].as_str().is_some_and(|s| !s.is_empty()),
                "Incomplete PayPal account configuration"
            );
        }
        assert!(
            view::<Attribution>(account)
                .bn_code
                .as_str()
                .unwrap()
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_'),
            "Invalid provider attribution configuration"
        );
    }
    for versions in services
        .as_object()
        .expect("Payment services object")
        .values()
    {
        for version in versions
            .as_object()
            .expect("Payment service versions object")
            .values()
        {
            let service: ServiceEntry = view(version);
            if service.url.is_string() {
                validate_service(
                    version,
                    service
                        .environment
                        .as_str()
                        .expect("Payment environment required"),
                )
                .expect("Invalid payment service configuration");
            } else {
                let environments = version
                    .as_object()
                    .expect("Payment environment map required");
                assert!(
                    !environments.is_empty()
                        && environments.keys().all(|key| key == "defaultEnvironment"
                            || ["sandbox", "live", "simulation", "contract-fixture"]
                                .contains(&key.as_str())),
                    "Payment environment map must declare supported environments"
                );
                if let Some(default) = version.get("defaultEnvironment") {
                    assert!(
                        default.as_str().is_some_and(|value| [
                            "sandbox",
                            "live",
                            "simulation",
                            "contract-fixture"
                        ]
                        .contains(&value)
                            && version[value].is_object()),
                        "Default payment environment must be configured"
                    );
                }
                for environment in ["sandbox", "live", "simulation", "contract-fixture"] {
                    if let Some(config) = version.get(environment) {
                        validate_service(config, environment)
                            .expect("Invalid payment service environment configuration");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_accounts, validate_service};
    use serde_json::json;
    #[test]
    fn hosted_simulation_requires_https_and_a_configured_default() {
        let service = json!({"url":"https://payments.example.test","token":"synthetic-credential-with-32-characters","environment":"simulation","approvalOrigins":["https://payments.example.test"]});
        validate_accounts(
            &json!({}),
            &json!({"provider":{"1.2.0":{"defaultEnvironment":"simulation","simulation":service}}}),
        );
        assert!(
            std::panic::catch_unwind(|| validate_accounts(
                &json!({}),
                &json!({"provider":{"1.2.0":{"defaultEnvironment":"live","simulation":service}}})
            ))
            .is_err()
        );
        let mut insecure = service;
        insecure["url"] = json!("http://127.0.0.1:8888");
        assert!(validate_service(&insecure, "simulation").is_err());
    }
}
