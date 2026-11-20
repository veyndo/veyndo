//! Admission of an adapter service configuration and of the approval URLs it may hand to a buyer.
use crate::domain::attempt::Attempt;
use crate::ports::Host;
use serde_json::Value;
use veyndo_core::{Error, Result, StatusCode, bad};

pub fn validate_service(c: &Value, environment: &str) -> Result<()> {
    let url = reqwest::Url::parse(c["url"].as_str().ok_or(Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Payment service version unavailable".into(),
    ))?)
    .map_err(|_| bad("Invalid payment service URL"))?;
    let fixture = url.scheme() == "http"
        && ["127.0.0.1", "localhost"].contains(&url.host_str().unwrap_or(""));
    if !["sandbox", "live", "simulation", "contract-fixture"].contains(&environment)
        || (!fixture && url.scheme() != "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || c["environment"] != environment
        || (fixture && environment != "contract-fixture")
        || c["token"].as_str().is_none_or(|v| v.len() < 24)
    {
        return Err(bad(
            "Payment service requires matching environment, HTTPS and a private credential",
        ));
    }
    Ok(())
}
pub fn approval_url(host: &dyn Host, p: &Attempt, value: &Value) -> Result<Option<String>> {
    let Some(s) = value.as_str() else {
        if value.is_null() {
            return Ok(None);
        }
        return Err(bad("Invalid approval URL"));
    };
    let u = reqwest::Url::parse(s).map_err(|_| bad("Invalid provider approval URL"))?;
    let c = host.service(&p.provider, &p.adapter_version, &p.environment)?;
    if !approval_transport(&u, &p.environment)
        || !u.username().is_empty()
        || u.password().is_some()
        || !c["approvalOrigins"].as_array().is_some_and(|origins| {
            origins
                .iter()
                .any(|v| v.as_str() == Some(u.origin().ascii_serialization().as_str()))
        })
    {
        return Err(bad("Provider approval origin is not authorized"));
    }
    Ok(Some(s.into()))
}

// Local browser acceptance uses a separate loopback provider frame. Sandbox and Live keep HTTPS.
fn approval_transport(url: &reqwest::Url, environment: &str) -> bool {
    url.scheme() == "https"
        || (environment == "contract-fixture"
            && url.scheme() == "http"
            && ["127.0.0.1", "localhost"].contains(&url.host_str().unwrap_or("")))
}
#[cfg(test)]
mod tests {
    use super::{approval_transport, approval_url, validate_service};
    use crate::domain::attempt::sample;
    use crate::ports::Host;
    use serde_json::{Value, json};
    use veyndo_core::{PortFuture, Result};

    struct Fixed;
    impl Host for Fixed {
        fn service(&self, _: &str, _: &str, _: &str) -> Result<Value> {
            Ok(json!({"approvalOrigins":["https://pay.example"]}))
        }
        fn captured<'a, 'c: 'a>(
            &'a self,
            _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
            _: &'a str,
            _: &'a mut Value,
            _: &'a str,
        ) -> PortFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }
    }

    #[test]
    fn a_service_needs_its_environment_https_and_a_private_credential() {
        let token = "t".repeat(24);
        let c = json!({"url":"https://adapter.example/v1","environment":"live","token":token});
        assert!(validate_service(&c, "live").is_ok());
        assert!(validate_service(&c, "sandbox").is_err());
        assert!(validate_service(&c, "production").is_err());
        assert!(validate_service(&json!({}), "live").is_err());
        for (key, value) in [
            ("url", json!("http://adapter.example/v1")),
            ("url", json!("https://user:secret@adapter.example/v1")),
            ("url", json!("https://adapter.example/v1?x=1")),
            ("url", json!("https://adapter.example/v1#x")),
            ("url", json!("not a url")),
            ("token", json!("short")),
            ("token", Value::Null),
        ] {
            let mut changed = c.clone();
            changed[key] = value;
            assert!(validate_service(&changed, "live").is_err(), "{key}");
        }
        let local =
            json!({"url":"http://127.0.0.1:9000","environment":"contract-fixture","token":token});
        assert!(validate_service(&local, "contract-fixture").is_ok());
        let mut live = local;
        live["environment"] = json!("live");
        assert!(validate_service(&live, "live").is_err());
    }

    #[test]
    fn approval_urls_stay_on_the_origins_the_service_authorized() {
        let p = sample();
        assert_eq!(approval_url(&Fixed, &p, &Value::Null).ok(), Some(None));
        assert_eq!(
            approval_url(&Fixed, &p, &json!("https://pay.example/approve?id=1")).ok(),
            Some(Some("https://pay.example/approve?id=1".into()))
        );
        for invalid in [
            json!(7),
            json!("not a url"),
            json!("https://other.example/approve"),
            json!("http://pay.example/approve"),
            json!("https://user:secret@pay.example/approve"),
        ] {
            assert!(approval_url(&Fixed, &p, &invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn local_provider_frames_require_fixture_environment_and_exact_loopback() {
        for origin in [
            "http://127.0.0.1:9000/checkout",
            "http://localhost:9000/checkout",
        ] {
            let url = reqwest::Url::parse(origin).unwrap();
            assert!(approval_transport(&url, "contract-fixture"));
            for environment in ["live", "sandbox", "simulation", ""] {
                assert!(!approval_transport(&url, environment));
            }
        }
        for origin in [
            "http://provider.example/",
            "http://localhost.example/",
            "http://10.0.0.1/",
            "javascript:alert(1)",
            "file:///checkout",
        ] {
            assert!(!approval_transport(
                &reqwest::Url::parse(origin).unwrap(),
                "contract-fixture"
            ));
        }
        assert!(approval_transport(
            &reqwest::Url::parse("https://provider.example/checkout").unwrap(),
            "live"
        ));
    }
}
