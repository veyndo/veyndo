//! Optional trusted ingress geography. Ordinary country headers never establish visitor location.
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, hash, process_config};
/// Whether a trusted ingress may tell the visitor country.
pub fn configured() -> bool {
    process_config::get().behavior_geo_secret.is_some()
}
/// The visitor country a trusted ingress proved; ordinary headers never count.
pub fn country(h: &RequestContext) -> Option<String> {
    let secret = process_config::get().behavior_geo_secret.as_deref()?;
    let supplied = header(h, "x-veyndo-geo-token")?;
    if hash(supplied) != hash(secret) {
        return None;
    }
    let country = header(h, "x-veyndo-geo-country")?;
    // Reuse the bundled world catalogue rather than accepting invented country codes.
    veyndo_shipping::domain::geography::world()
        .iter()
        .find(|c| c.iso_assigned && c.code == country)
        .map(|c| c.code.clone())
}

#[cfg(test)]
mod tests {
    use super::{configured, country};
    use serde_json::json;
    use veyndo_core::RequestContext;
    use veyndo_core::process_config::{Config, init};

    fn request(token: Option<&str>, code: Option<&str>) -> RequestContext {
        let mut h = RequestContext::new();
        if let Some(token) = token {
            h.insert("x-veyndo-geo-token", token.parse().unwrap());
        }
        if let Some(code) = code {
            h.insert("x-veyndo-geo-country", code.parse().unwrap());
        }
        h
    }

    #[test]
    fn only_the_trusted_ingress_tells_an_assigned_country() {
        init(|| Config {
            strict: false,
            http_api_only: false,
            shop_domain_suffix: String::new(),
            listener_url: String::new(),
            allow_bootstrap: false,
            paypal_accounts: json!({}),
            webhook_keys: json!({}),
            paypal_base: String::new(),
            paypal_environment: "sandbox",
            checkout_paypal_sandbox: false,
            payment_services: json!({}),
            frame_ancestors: String::new(),
            services: json!({}),
            publishers: json!({}),
            private_service_origins: String::new(),
            outbox_retention_days: 30,
            outbox_metadata_days: 30,
            behavior_geo_secret: Some("ingress-secret".into()),
            trusted_proxies: vec![],
        });
        assert!(configured());
        assert_eq!(
            country(&request(Some("ingress-secret"), Some("DE"))).as_deref(),
            Some("DE")
        );
        assert_eq!(country(&request(Some("guessed"), Some("DE"))), None);
        assert_eq!(country(&request(None, Some("DE"))), None);
        assert_eq!(country(&request(Some("ingress-secret"), None)), None);
        assert_eq!(country(&request(Some("ingress-secret"), Some("ZZ"))), None);
        assert_eq!(country(&request(Some("ingress-secret"), Some("de"))), None);
    }
}
