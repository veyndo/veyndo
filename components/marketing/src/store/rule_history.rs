//! Rule comparison on bounded, tenant-owned frozen Flow order inputs; it never reads current commerce data.
use crate::domain::rule_comparison::{digest, evaluate};
use crate::domain::rules;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_app_model::declarations::identifier;
use veyndo_core::{Error, Result, State, StatusCode, bad, conflict};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    rule_id: String,
    revision: i64,
    flow_id: String,
    condition: rules::Condition,
    limit: u32,
}

/// Compares a saved rule with a draft on the most recent frozen `order.placed` inputs of one Flow.
pub async fn compare(a: &State, t: &str, v: Value) -> Result<Value> {
    if v.to_string().len() > 32_768 {
        return Err(bad("Rule comparison exceeds 32 KiB"));
    }
    let input: Input = serde_json::from_value(v).map_err(|_| bad("Invalid rule comparison"))?;
    if !identifier(&input.rule_id)
        || !identifier(&input.flow_id)
        || input.revision < 1
        || !(1..=50).contains(&input.limit)
    {
        return Err(bad("Saved rule, Flow, revision and limit (1–50) required"));
    }
    input.condition.validate(0)?;
    let row = sqlx::query(
        "SELECT condition,revision,active FROM commerce_rules WHERE tenant=$1 AND id=$2",
    )
    .bind(t)
    .bind(&input.rule_id)
    .fetch_optional(&a.db)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Rule unavailable".into()))?;
    if row.get::<i64, _>("revision") != input.revision {
        return Err(conflict("Configuration revision changed"));
    }
    let baseline: rules::Condition =
        serde_json::from_value(row.get("condition")).map_err(|_| bad("Invalid saved rule"))?;
    baseline.validate(0)?;
    // Existing (tenant,flow,event_id) index bounds the sample. Never fetch current carts,
    // products/customers/references or dispatch any Flow action during comparison.
    let rows = sqlx::query("SELECT id,event_id,CASE WHEN octet_length(definition::text)<=262144 THEN definition ELSE NULL END AS snapshot FROM flow_jobs WHERE tenant=$1 AND flow=$2 ORDER BY event_id DESC LIMIT $3")
        .bind(t).bind(&input.flow_id).bind(i64::from(input.limit)).fetch_all(&a.db).await?;
    let results: Vec<Value> = rows
        .iter()
        .map(|r| {
            let snapshot: Option<Value> = r.get("snapshot");
            let mut result = match snapshot {
                Some(d) => evaluate(t, &d, &baseline, &input.condition),
                None => json!({"status":"unavailable","reason":"snapshot_too_large"}),
            };
            result["jobId"] = json!(r.get::<String, _>("id"));
            result["eventId"] = json!(r.get::<i64, _>("event_id"));
            result
        })
        .collect();
    let evaluated = results
        .iter()
        .filter(|r| r["status"] == "evaluated")
        .count();
    let changed = results
        .iter()
        .filter(|r| r["status"] == "evaluated" && r["before"] != r["after"])
        .count();
    Ok(
        json!({"sideEffects":false,"comparisonVersion":1,"ruleId":input.rule_id,
        "revision":input.revision,"baselineActive":row.get::<bool,_>("active"),"flowId":input.flow_id,
        "baselineDigest":digest(&json!(baseline)),"candidateDigest":digest(&json!(input.condition)),
        "sampled":results.len(),"evaluated":evaluated,"changed":changed,
        "unavailable":results.len()-evaluated,"results":results}),
    )
}
