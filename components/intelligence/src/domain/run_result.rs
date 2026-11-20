//! The synchronous planning result of a saved run: a ready proposal, or the error the run stopped with.
use serde_json::{Value, json};
use veyndo_core::{Error, Result, StatusCode};

/// Preserve the synchronous planning error contract while the saved run remains inspectable.
pub fn plan_result(run: Value) -> Result<Value> {
    if run["state"] == "ready" {
        return Ok(
            json!({"taskId":run["taskId"],"preview":run["preview"],"approvalRequired":true,"run":run}),
        );
    }
    if [Some("failed"), Some("uncertain"), Some("cancelled")].contains(&run["state"].as_str()) {
        let status = run["error"]["status"]
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())
            .and_then(|v| StatusCode::from_u16(v).ok())
            .filter(|v| v.is_client_error() || v.is_server_error())
            .unwrap_or(StatusCode::BAD_GATEWAY);
        return Err(Error(
            status,
            format!(
                "Saved agent run {} stopped: {}. Inspect its status before continuing.",
                run["taskId"].as_str().unwrap_or(""),
                run["error"]["detail"]
                    .as_str()
                    .unwrap_or("explicit review required")
            ),
        ));
    }
    Ok(run)
}
