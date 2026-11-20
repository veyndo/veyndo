//! Bounded flow execution summaries of a shop; a restricted reader never sees action output.
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Result, State};

/// The newest fifty jobs; `full` is true only for a reader holding every scope.
pub async fn values(a: &State, t: &str, full: bool) -> Result<Value> {
    let jobs=sqlx::query("SELECT j.id,j.flow,j.state,j.result,j.error,j.cursor,j.execution,to_char(j.available_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS available_at,COALESCE(t.applied,false) AS applied,COALESCE(t.run_request ? 'execution',false) AS execution_started FROM flow_jobs j LEFT JOIN tasks t ON t.tenant=j.tenant AND t.id=j.result->>'taskId' WHERE j.tenant=$1 ORDER BY j.event_id DESC,j.id LIMIT 50").bind(t).fetch_all(&a.db).await?;
    let value=json!(jobs.iter().map(|r|json!({"id":r.get::<String,_>("id"),"flow":r.get::<String,_>("flow"),"state":r.get::<String,_>("state"),"result":r.get::<Option<Value>,_>("result"),"error":r.get::<Option<String>,_>("error"),"cursor":r.get::<Option<String>,_>("cursor"),"execution":r.get::<Value,_>("execution"),"availableAt":r.get::<String,_>("available_at"),"applied":r.get::<bool,_>("applied"),"executionStarted":r.get::<bool,_>("execution_started")})).collect::<Vec<_>>());
    Ok(project(value, full))
}

/// A trace can contain private app output or model evidence; summaries never replay that authority.
fn project(mut value: Value, full: bool) -> Value {
    if !full && let Some(jobs) = value.as_array_mut() {
        for j in jobs {
            j["result"] = Value::Null;
            if !j["error"].is_null() {
                j["error"] =
                    json!("Flow action failed; detailed output requires source permissions");
            }
            let trace=j["execution"]["trace"].as_array().map(|rows| rows.iter().map(|r|json!({"node":r["node"],"action":r["action"],"matched":r["matched"]})).collect::<Vec<_>>()).unwrap_or_default();
            j["execution"] = json!({"trace":trace});
        }
    }
    value
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
