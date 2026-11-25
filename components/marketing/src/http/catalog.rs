//! Native condition metadata, app action/event discovery and source-compatible condition import.
use crate::domain::condition_import::normalize;
use crate::domain::rule_fields::FIELDS;
use crate::domain::rules::Condition;
use crate::store::catalog;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_automation::rules as automation_rules;
use veyndo_core::document::view;
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
    let rows = catalog::active_apps(&a, &t).await?;
    let rules = catalog::active_rules(&a, &t).await?;
    let source: SourceCatalog = view(automation_rules::catalog());
    Ok(Json(
        json!({"upstream":"shopware/core 6.7.14.2","conditions":conditions(),"fields":FIELDS,"events":builtin::EVENTS,"apps":rows.iter().map(|r|json!({"id":r.id,"manifest":r.manifest})).collect::<Vec<_>>(),"completeUpstreamConditionCatalog":false,"rules":rules.iter().map(|r|json!({"id":r.id,"name":r.name,"revision":r.revision})).collect::<Vec<_>>(),"sourceConditions":source.conditions,"actions":builtin::PIPELINE_ACTIONS,"sourceActions":source.actions,"flowCatalogVersion":builtin::VERSION,"pipelineContract":{"nodes":["condition","action","delay","stop"],"maxNodes":veyndo_flow_graph::MAX_NODES,"maxDelaySeconds":2592000,"cyclicGraphs":false}}),
    ))
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

/// The upstream condition and action catalogue of the automation rules.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SourceCatalog {
    conditions: Value,
    actions: Value,
}
