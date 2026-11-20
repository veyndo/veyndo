//! Service job requests reuse the existing declared action gateway, actor checks, quotas and transactional outbox.
use crate::domain::tool_output::checked_output;
use crate::domain::{action_admission, callback_grant, input_schema};
use crate::store::jobs;
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};

/// Queues a declared job for a service that asks with its own request key.
pub async fn execute(a: &State, h: &RequestContext, m: &Manifest, v: &Value) -> Result<Value> {
    let contract: Value = serde_json::from_str(include_str!(
        "../../../../extensions/sdk/host-exchange-v1.json"
    ))
    .expect("bundled host exchange contract");
    input_schema::validate(&contract["operations"]["job_enqueue"]["inputSchema"], v)?;
    if !m.permissions.iter().any(|p| p == "jobs.write") {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "App job permission required".into(),
        ));
    }
    callback_grant::permit(h, "jobs.write")?;
    let name = v["action"].as_str().unwrap();
    let key = v["requestKey"].as_str().unwrap();
    if key.is_empty() || key.chars().any(char::is_control) {
        return Err(bad(
            "A nonempty job request key without control characters is required",
        ));
    }
    let mut context = h.clone();
    context.insert(
        "idempotency-key",
        key.parse().map_err(|_| bad("Invalid job request key"))?,
    );
    // Preserve the authenticated principal. The gateway rechecks every action right,
    // schema, live-only admission and current package before creating the native job.
    invoke_job(a, &context, m, name, &v["input"]).await
}
// A job callback holds its admitted manifest/action through enqueue. Re-reading
// the package must never turn a formerly declared job into a service invocation.
/// Queues a declared job action of an already admitted package.
pub async fn invoke_job(
    a: &State,
    h: &RequestContext,
    m: &Manifest,
    name: &str,
    v: &Value,
) -> Result<Value> {
    let action = m
        .actions
        .iter()
        .find(|action| action.name == name && action.handler == "job")
        .ok_or(bad("Callback requires a declared job action"))?;
    action_admission::check(h, action, v)?;
    checked_output(action, jobs::enqueue(a, h, m, action, v).await?)
}
