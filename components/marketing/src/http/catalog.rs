//! Native condition metadata, app action/event discovery and source-compatible condition import.
use crate::domain::condition_import::normalize;
use crate::domain::rule_fields::FIELDS;
use crate::domain::rules::Condition;
use axum::Json;
use axum::extract::State as Shared;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_automation::rules as automation_rules;
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_flow_graph::catalog as builtin;
use veyndo_identity::{merchant, permit};

/// The native conditions, rule references and rule fields.
fn conditions() -> Value {
    json!([
        "andContainer",
        "orContainer",
        "notContainer",
        "alwaysValid",
        "cartCartAmount",
        "cartLineItemCount",
        "customerGroup",
        "shippingCountry",
        "salesChannel",
        "lineItemId",
        "customerLoggedIn",
        "orderState",
        "paymentState",
        "deliveryState",
        "contextField",
        "eventField",
        "shippingMethod",
        "paymentMethod",
        "customerEmail",
        "customerBillingCountry",
        "customerShippingCountry",
        "cartLineItem",
        "ruleReference"
    ])
}

/// `GET /api/automation/catalog`: conditions, events, actions and the installed apps a flow can use.
pub async fn catalog(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    permit(&h, "automation.read")?;
    let t = merchant(&h)?;
    let rows =
        sqlx::query("SELECT id,manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id")
            .bind(&t)
            .fetch_all(&a.db)
            .await?;
    let mut result = json!({"upstream":"shopware/core 6.7.14.2","conditions":conditions(),"fields":FIELDS,"events":builtin::EVENTS,"apps":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"manifest":r.get::<Value,_>("manifest")})).collect::<Vec<_>>(),"completeUpstreamConditionCatalog":false});
    let rules=sqlx::query("SELECT id,name,revision FROM commerce_rules WHERE tenant=$1 AND active ORDER BY id LIMIT 100").bind(&t).fetch_all(&a.db).await?;
    result["rules"]=json!(rules.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<Value,_>("name"),"revision":r.get::<i64,_>("revision")})).collect::<Vec<_>>());
    let source = automation_rules::catalog();
    result["sourceConditions"] = source["conditions"].clone();
    result["actions"] = json!(builtin::PIPELINE_ACTIONS);
    result["sourceActions"] = source["actions"].clone();
    result["flowCatalogVersion"] = json!(builtin::VERSION);
    result["pipelineContract"] = json!({"nodes":["condition","action","delay","stop"],"maxNodes":veyndo_flow_graph::MAX_NODES,"maxDelaySeconds":2592000,"cyclicGraphs":false});
    Ok(Json(result))
}

/// `POST /api/automation/import-condition`: normalizes a source condition and checks it, without saving it.
pub async fn import(h: RequestContext, Json(v): Json<Value>) -> Result<Json<Value>> {
    permit(&h, "automation.manage")?;
    let normalized = normalize(&v, 0)?;
    let c: Condition = serde_json::from_value(normalized.clone())
        .map_err(|_| bad("Unsupported Shopware condition"))?;
    c.validate(0)?;
    Ok(Json(json!({"condition":normalized,"saved":false})))
}
