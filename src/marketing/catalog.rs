//! Native condition metadata, app action/event discovery and source-compatible condition import.
use super::*;
use veyndo_flow_graph::catalog as builtin;
use veyndo_marketing::domain::condition_import::normalize;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/automation/catalog",
            &[("GET", "automation.read")],
            get(catalog),
        )
        .secure_route(
            "/api/automation/import-condition",
            &[("POST", "automation.manage")],
            post(import),
        )
}
pub(super) async fn catalog(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    auth::permit(&h, "automation.read")?;
    let t = merchant(&a, &h)?;
    let rows =
        sqlx::query("SELECT id,manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id")
            .bind(t)
            .fetch_all(&a.db)
            .await?;
    let mut result = json!({"upstream":"shopware/core 6.7.14.2","conditions":["andContainer","orContainer","notContainer","alwaysValid","cartCartAmount","cartLineItemCount","customerGroup","shippingCountry","salesChannel","lineItemId","customerLoggedIn","orderState","paymentState","deliveryState","contextField","eventField","shippingMethod","paymentMethod","customerEmail","customerBillingCountry","customerShippingCountry","cartLineItem"],"fields":rule_fields::FIELDS,"events":builtin::EVENTS,"apps":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"manifest":r.get::<Value,_>("manifest")})).collect::<Vec<_>>(),"completeUpstreamConditionCatalog":false});
    let rules=sqlx::query("SELECT id,name,revision FROM commerce_rules WHERE tenant=$1 AND active ORDER BY id LIMIT 100").bind(merchant(&a,&h)?).fetch_all(&a.db).await?;
    result["rules"]=json!(rules.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<Value,_>("name"),"revision":r.get::<i64,_>("revision")})).collect::<Vec<_>>());
    result["conditions"]
        .as_array_mut()
        .unwrap()
        .push(json!("ruleReference"));
    result["sourceConditions"] = veyndo::automation_rules::catalog()["conditions"].clone();
    result["actions"] = json!(builtin::PIPELINE_ACTIONS);
    result["sourceActions"] = veyndo::automation_rules::catalog()["actions"].clone();
    result["flowCatalogVersion"] = json!(builtin::VERSION);
    result["pipelineContract"] = json!({"nodes":["condition","action","delay","stop"],"maxNodes":veyndo_flow_graph::MAX_NODES,"maxDelaySeconds":2592000,"cyclicGraphs":false});
    Ok(Json(result))
}
pub(super) async fn import(h: RequestContext, Json(v): Json<Value>) -> Result<Json<Value>> {
    auth::permit(&h, "automation.manage")?;
    let normalized = normalize(&v, 0)?;
    let c: rules::Condition = serde_json::from_value(normalized.clone())
        .map_err(|_| bad("Unsupported Shopware condition"))?;
    c.validate(0)?;
    Ok(Json(json!({"condition":normalized,"saved":false})))
}
