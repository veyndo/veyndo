//! Generated conformance driver; invokes the same production policy functions as the commerce server.
use super::Request;
use serde_json::{Value, json};
use veyndo::verified_kernel::*;
pub(super) fn eval(request: &Request) -> Result<Value, String> {
    let args = &request.args;
    let b = |key: &str| args[key].as_bool().ok_or_else(|| format!("Invalid {key}"));
    match request.function.as_deref() {
        Some("planning_request_admissible") => Ok(json!(planning_request_admissible(
            b("read_only")?,
            b("planning_post")?
        ))),
        Some("agent_preview_admissible") => Ok(json!(agent_preview_admissible(
            b("active")?,
            b("private_preview")?,
            b("personal")?,
            b("same_owner")?,
            b("current")?
        ))),
        _ => Err("Unknown policy".into()),
    }
}
