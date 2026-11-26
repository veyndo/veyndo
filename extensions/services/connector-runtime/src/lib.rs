//! The shared runtime of the standard app services: errors, tenant-scoped encrypted storage and source exports, the delivery queue, pinned network egress, the OAuth flow and the contract types app providers implement.
pub mod crypto;
pub mod error;
mod exports;
pub mod network;
pub mod oauth;
pub mod queue;
pub mod store;
pub use error::{Error, Result};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use std::{env, time::Duration};
pub use store::Store;

/// The OAuth policy an app declares: environment prefix of its client, scope, Slack token shape and provider endpoints.
pub struct OAuth {
    pub prefix: &'static str,
    pub scope: &'static str,
    pub slack: bool,
    pub authorize: &'static str,
    pub token: &'static str,
}

/// The settings an OAuth app accepts: free keys, booleans, validated identifiers and its optional export cursor.
pub struct Settings {
    pub keys: &'static [&'static str],
    pub booleans: &'static [&'static str],
    pub identifiers: &'static [(&'static str, &'static str)],
    pub cursor_field: Option<&'static str>,
}

/// One prepared API-key integration call: URL, JSON body and provider headers.
pub struct Call {
    pub url: String,
    pub body: Option<Value>,
    pub headers: Vec<(&'static str, String)>,
}

pub fn digest(value: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(value))
}
pub fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}
pub fn checked(condition: bool, message: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(message))
    }
}

pub fn identifier(v: &Value, k: &str) -> Result<String> {
    let s = text(v, k);
    checked(
        regex::Regex::new(r"^[a-zA-Z0-9_-]{1,100}$")
            .unwrap()
            .is_match(&s),
        "Missing or invalid provider identifier",
    )?;
    Ok(s)
}
pub fn secret(key: &str) -> Result<()> {
    checked(
        !key.is_empty() && key.len() <= 4000 && !key.contains(['\r', '\n', '\0']),
        "Provider credential required",
    )
}
pub fn short(v: &Value, k: &str, max: usize) -> Result<String> {
    let s = text(v, k);
    checked(
        !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control),
        "Missing or invalid operation field",
    )?;
    Ok(s)
}
pub fn bearer(key: &str) -> Vec<(&'static str, String)> {
    vec![("authorization", format!("Bearer {key}"))]
}
