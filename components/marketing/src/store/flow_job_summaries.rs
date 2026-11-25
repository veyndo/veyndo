//! Bounded flow execution summaries of a shop; a restricted reader never sees action output.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, lenient_list, view};
use veyndo_core::{Result, State};

/// The newest fifty jobs; `full` is true only for a reader holding every scope.
pub async fn values(a: &State, t: &str, full: bool) -> Result<Value> {
    let jobs=sqlx::query!("SELECT j.id,j.flow,j.state,j.result,j.error,j.cursor,j.execution,to_char(j.available_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS available_at,COALESCE(t.applied,false) AS applied,COALESCE(t.run_request ? 'execution',false) AS execution_started FROM flow_jobs j LEFT JOIN tasks t ON t.tenant=j.tenant AND t.id=j.result->>'taskId' WHERE j.tenant=$1 ORDER BY j.event_id DESC,j.id LIMIT 50", t).fetch_all(&a.db).await?;
    let value=json!(jobs.iter().map(|r|json!({"id":r.id.clone(),"flow":r.flow.clone(),"state":r.state.clone(),"result":r.result.clone(),"error":r.error.clone(),"cursor":r.cursor.clone(),"execution":r.execution.clone(),"availableAt":r.available_at.clone(),"applied":r.applied.clone(),"executionStarted":r.execution_started.clone()})).collect::<Vec<_>>());
    Ok(project(value, full))
}

/// A trace can contain private app output or model evidence; summaries never replay that authority.
fn project(mut value: Value, full: bool) -> Value {
    if !full && let Some(jobs) = value.as_array_mut() {
        for j in jobs {
            let job: JobTrace = view(j);
            j["result"] = Value::Null;
            if !job.error.is_null() {
                j["error"] =
                    json!("Flow action failed; detailed output requires source permissions");
            }
            let trace = job
                .execution
                .trace
                .iter()
                .map(|r| json!({"node":r.node,"action":r.action,"matched":r.matched}))
                .collect::<Vec<_>>();
            j["execution"] = json!({"trace":trace});
        }
    }
    value
}

/// A job summary's error and the steps of its trace.
#[derive(Deserialize, Default)]
#[serde(default)]
struct JobTrace {
    error: Value,
    #[serde(deserialize_with = "lenient")]
    execution: TraceSteps,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TraceSteps {
    #[serde(deserialize_with = "lenient_list")]
    trace: Vec<TraceStep>,
}

/// One trace step without the action's output.
#[derive(Deserialize, Default)]
#[serde(default)]
struct TraceStep {
    node: Value,
    action: Value,
    matched: Value,
}

#[cfg(test)]
mod tests {
    use super::project;
    use serde_json::json;
    #[test]
    fn restricted_trace_never_contains_action_output() {
        let jobs = json!([{"result":{"customerEmail":"private@example.test"},"error":"private detail","execution":{"trace":[{"node":"one","action":"app","result":{"secret":"private"}}]}}]);
        let masked = project(jobs.clone(), false);
        assert!(masked[0]["result"].is_null());
        assert!(masked[0]["execution"]["trace"][0]["result"].is_null());
        assert!(!masked.to_string().contains("private"));
        assert_eq!(project(jobs.clone(), true), jobs);
    }
}
