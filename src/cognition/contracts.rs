//! Evidence APIs and MCP contracts dispatch into one authorized owner; the claim compiler rejects free prose.
use super::*;
pub(crate) fn schema(name: &str) -> Option<Value> {
    if let Some(schema) = super::behavior::schema(name) {
        return Some(schema);
    }
    if let Some(schema) = experiments_schema(name) {
        return Some(schema);
    }
    let props = match name {
        "catalog.facts" => json!({"productId":{"type":"string"}}),
        "knowledge.autonomy.apply" => json!({"taskId":{"type":"string"}}),
        "knowledge.autonomy.status" => json!({}),
        "knowledge.autonomy.revert" => {
            json!({"approve":{"type":"boolean"},"productId":{"type":"string","maxLength":160}})
        }
        "knowledge.claims" => json!({"productId":{"type":"string"}}),
        "knowledge.claim.propose" => {
            json!({"productId":{"type":"string"},"sourceId":{"type":"string"},"contentHash":{"type":"string"},"text":{"type":"string","maxLength":1200},"quote":{"type":"string","maxLength":2400},"locale":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1},"nodeType":{"type":"string","enum":evidence::NODE_TYPES}})
        }
        "knowledge.claim.review" => {
            json!({"id":{"type":"string"},"revision":{"type":"integer"},"state":{"type":"string","enum":["evidenced","confirmed","rejected"]},"approve":{"type":"boolean"}})
        }
        "knowledge.extract" => {
            json!({"sourceId":{"type":"string"},"productId":{"type":"string"},"locale":{"type":"string"},"inference":{"type":"object"}})
        }
        "knowledge.compile" => {
            json!({"productId":{"type":"string"},"claims":{"type":"array","maxItems":20,"items":{"type":"object","properties":{"id":{"type":"string"},"text":{"type":"string"}},"required":["id","text"],"additionalProperties":false}}})
        }
        _ => return None,
    };
    Some(json!({"type":"object","properties":props,"additionalProperties":false}))
}
pub(crate) async fn invoke_contract(
    a: &App,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    if super::behavior::schema(name).is_some() {
        return super::behavior::invoke(a, h, name, v).await;
    }
    let t = merchant(a, h)?;
    auth::permit(h, "knowledge.read")?;
    if name == "knowledge.experiments" || name.starts_with("knowledge.experiment.") {
        return experiments::invoke(a, h, name, v).await;
    }
    let product = || v["productId"].as_str().ok_or(bad("Product required"));
    match name {
        "knowledge.autonomy.apply" => {
            crate::proposal_apply::apply_mode(
                a,
                &t,
                v["taskId"].as_str().ok_or(bad("Task required"))?,
                h,
                true,
            )
            .await
        }
        "knowledge.autonomy.status" => autonomy::status(a, &t, h).await,
        "knowledge.autonomy.revert" => autonomy_revert::request(a, &t, h, v).await,
        "knowledge.claims" => evidence::claims(a, &t, product()?, false).await,
        "knowledge.claim.propose" => evidence::propose(a, h, v).await,
        "knowledge.claim.review" => evidence::decide(a, h, v).await,
        "knowledge.extract" => extraction::extract(a, h, v).await,
        "knowledge.compile" => compile(a, &t, product()?, v).await,
        _ => Err(bad("Unknown intelligence contract")),
    }
}
pub(crate) use veyndo_intelligence::service::fact_compiler::compile;
use veyndo_intelligence::service::fact_compiler::{
    experiments_schema, public_compile, public_facts,
};
async fn api(
    State(a): State<App>,
    h: RequestContext,
    Path(name): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        invoke_contract(&a, &h, &format!("knowledge.{name}"), &v).await?,
    ))
}
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/intelligence/{name}",
            &[("POST", "knowledge.read")],
            post(api),
        )
        .merge(super::claim_batches::router())
        .route("/store-api/product/{id}/facts", get(public_facts))
        .route(
            "/store-api/product/{id}/facts/compile",
            post(public_compile),
        )
}
