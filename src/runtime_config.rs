//! Immutable process configuration, validated once at startup. Mutable shop/provider settings remain in PostgreSQL.
use serde_json::Value;
use std::{env, sync::OnceLock};
pub(crate) struct Config {
    pub strict: bool,
    pub http_api_only: bool,
    pub shop_domain_suffix: String,
    pub listener_url: String,
    pub allow_bootstrap: bool,
    pub paypal_accounts: Value,
    pub webhook_keys: Value,
    pub paypal_base: String,
    pub paypal_environment: &'static str,
    pub checkout_paypal_sandbox: bool,
    pub payment_services: Value,
    pub frame_ancestors: String,
    pub services: Value,
    pub publishers: Value,
    pub private_service_origins: String,
    pub outbox_retention_days: u32,
    pub outbox_metadata_days: u32,
    pub behavior_geo_secret: Option<String>,
    pub trusted_proxies: Vec<std::net::IpAddr>,
}
pub(crate) fn get() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let strict = flag("DB_RLS_REQUIRED", false);
        let http_api_only = match env::var("HTTP_PROFILE").as_deref() {
            Ok("api-only") => true,
            Ok("combined") | Err(env::VarError::NotPresent) => false,
            _ => panic!("HTTP_PROFILE must be combined or api-only"),
        };
        let mode = env::var("BOOTSTRAP_MODE").unwrap_or("auto".into());
        assert!(
            !strict || mode == "serve",
            "Strict runtime must use BOOTSTRAP_MODE=serve; migrate in a separate owner job"
        );
        assert!(
            !strict || env::var("DATABASE_RUNTIME_URL").is_ok(),
            "Strict runtime URL required"
        );
        // Read once at startup so that an invalid default shop stops the process here, not on a request.
        veyndo_core::shop_scope::default_tenant();
        let services: Value =
            serde_json::from_str(&env::var("APP_SERVICES").unwrap_or("{}".into()))
                .expect("Invalid APP_SERVICES JSON");
        assert!(services.is_object(), "APP_SERVICES must be an object");
        let private_service_origins = env::var("APP_SERVICE_PRIVATE_ORIGINS").unwrap_or_default();
        for (_, service) in services.as_object().unwrap() {
            let parsed =
                reqwest::Url::parse(service["url"].as_str().expect("App service URL required"))
                    .expect("Invalid app service URL");
            assert!(
                crate::apps::service_origin_allowed(&parsed, &private_service_origins),
                "App service origin not approved"
            );
            if let Some(ui) = service["uiUrl"].as_str() {
                let ui = reqwest::Url::parse(ui).expect("Invalid app UI URL");
                assert!(
                    ui.username().is_empty()
                        && ui.password().is_none()
                        && (ui.scheme() == "https"
                            || ui.scheme() == "http"
                                && ["localhost", "127.0.0.1"]
                                    .contains(&ui.host_str().unwrap_or(""))),
                    "Unsafe app UI origin"
                );
            }
        }
        let trusted_proxies = env::var("TRUSTED_PROXY_IPS")
            .unwrap_or_default()
            .split(',')
            .filter(|x| !x.trim().is_empty())
            .map(|x| x.trim().parse().expect("Invalid TRUSTED_PROXY_IPS address"))
            .collect();
        let paypal_accounts = json_object("PAYPAL_ACCOUNTS", Some("PAYPAL_SANDBOX_ACCOUNTS"));
        let payment_services = json_object("PAYMENT_SERVICES", None);
        crate::payments::provider_configuration::validate_accounts(
            &paypal_accounts,
            &payment_services,
        );
        let frame_ancestors = env::var("STUDIO_FRAME_ORIGINS")
            .unwrap_or("https://app.veyndo.ai https://experience.veyndo.ai".into());
        for origin in frame_ancestors.split_whitespace() {
            let url = reqwest::Url::parse(origin).expect("Invalid STUDIO_FRAME_ORIGINS");
            assert!(
                url.origin().ascii_serialization() == origin
                    && url.username().is_empty()
                    && url.password().is_none()
                    && (url.scheme() == "https"
                        || url.scheme() == "http"
                            && ["localhost", "127.0.0.1"].contains(&url.host_str().unwrap_or(""))),
                "Invalid Studio frame origin"
            );
        }
        let database = veyndo_core::database::config::get();
        Config {
            http_api_only,
            shop_domain_suffix: env::var("SHOP_DOMAIN_SUFFIX").unwrap_or_default(),
            listener_url: database.listener_url.clone(),
            allow_bootstrap: flag("ALLOW_BOOTSTRAP_AUTH", true),
            paypal_accounts,
            webhook_keys: json_object("APP_WEBHOOK_KEYS", None),
            paypal_base: crate::payments::provider_configuration::paypal_base(),
            paypal_environment: crate::payments::provider_configuration::paypal_environment(),
            checkout_paypal_sandbox: flag("CHECKOUT_ENABLE_PAYPAL_SANDBOX_PURCHASE", false),
            payment_services,
            frame_ancestors,
            strict,
            services,
            publishers: json_object("APP_PUBLISHERS", None),
            private_service_origins,
            outbox_retention_days: number("OUTBOX_RETENTION_DAYS", 90, 1, 3650),
            outbox_metadata_days: number(
                "OUTBOX_METADATA_RETENTION_DAYS",
                365,
                number("OUTBOX_RETENTION_DAYS", 90, 1, 3650),
                36500,
            ),
            behavior_geo_secret: env::var("BEHAVIOR_GEO_SHARED_SECRET").ok().inspect(|v| {
                assert!(
                    (32..=256).contains(&v.len()),
                    "Geography ingress secret must contain 32–256 bytes"
                );
            }),
            trusted_proxies,
        }
    })
}
use veyndo_core::database::config::flag;
pub(crate) use veyndo_core::database::config::number;

fn json_object(key: &str, fallback: Option<&str>) -> Value {
    let raw = env::var(key)
        .or_else(|error| fallback.map(env::var).unwrap_or(Err(error)))
        .unwrap_or("{}".into());
    let value: Value = serde_json::from_str(&raw).unwrap_or_else(|_| panic!("Invalid {key} JSON"));
    assert!(value.is_object(), "{key} must be an object");
    value
}
