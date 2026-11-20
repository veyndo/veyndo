//! Immutable process configuration: the server validates it once at startup, every component reads the same value.
use serde_json::Value;
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

/// What the process was started with; mutable shop and provider settings remain in PostgreSQL.
pub struct Config {
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
/// Validates and installs the configuration on first use and returns it afterwards.
pub fn init(build: impl FnOnce() -> Config) -> &'static Config {
    CONFIG.get_or_init(build)
}

/// The installed configuration. The server installs it before it serves or works.
pub fn get() -> &'static Config {
    CONFIG
        .get()
        .expect("process configuration is installed at startup")
}
