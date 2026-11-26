//! The synchronous planning result of a saved run: a ready proposal, or the error the run stopped with.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, StatusCode};

/// Preserve the synchronous planning error contract while the saved run remains inspectable.
pub fn plan_result(run: Value) -> Result<Value> {
    let r: RunView = view(&run);
    if r.state == "ready" {
        return Ok(
            json!({"taskId":r.task_id,"preview":r.preview,"approvalRequired":true,"run":run}),
        );
    }
    if [Some("failed"), Some("uncertain"), Some("cancelled")].contains(&r.state.as_str()) {
        let status = r
            .error
            .status
            .and_then(|v| u16::try_from(v).ok())
            .and_then(|v| StatusCode::from_u16(v).ok())
            .filter(|v| v.is_client_error() || v.is_server_error())
            .unwrap_or(StatusCode::BAD_GATEWAY);
        return Err(Error(
            status,
            format!(
                "Saved agent run {} stopped: {}. Inspect its status before continuing.",
                r.task_id.as_str().unwrap_or(""),
                r.error
                    .detail
                    .as_deref()
                    .unwrap_or("explicit review required")
            ),
        ));
    }
    Ok(run)
}

/// The saved run fields the planning result reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RunView {
    state: Value,
    task_id: Value,
    preview: Value,
    #[serde(deserialize_with = "lenient")]
    error: RunError,
}

/// The problem a stopped run recorded.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RunError {
    #[serde(deserialize_with = "lenient")]
    status: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::plan_result;
    use serde_json::json;
    use veyndo_core::StatusCode;

    #[test]
    fn a_ready_run_is_a_proposal_that_needs_approval() {
        let run = json!({"state":"ready","taskId":"t1","preview":{"changes":1}});
        let v = plan_result(run.clone()).unwrap();
        assert_eq!(v["taskId"], "t1");
        assert_eq!(v["preview"], json!({"changes":1}));
        assert_eq!(v["approvalRequired"], true);
        assert_eq!(v["run"], run);
    }

    #[test]
    fn a_stopped_run_keeps_its_error_status_and_a_running_one_is_returned_as_is() {
        for state in ["failed", "uncertain", "cancelled"] {
            let e = plan_result(json!({"state":state,"taskId":"t1","error":{"status":409,"detail":"stale revision"}})).unwrap_err();
            assert_eq!(e.0, StatusCode::CONFLICT);
            assert!(
                e.1.contains("t1") && e.1.contains("stale revision"),
                "{}",
                e.1
            );
        }
        for status in [json!(200), json!(99999), json!("409"), json!(null)] {
            let e = plan_result(json!({"state":"failed","error":{"status":status}})).unwrap_err();
            assert_eq!(e.0, StatusCode::BAD_GATEWAY);
            assert!(e.1.contains("explicit review required"));
        }
        let running = json!({"state":"running","taskId":"t1"});
        assert_eq!(plan_result(running.clone()).ok(), Some(running));
    }
}
