//! Read-only filter planning through the central provider and fleet leases; execution remains native list APIs.
use crate::domain::grid_criteria::{schema as criteria_schema, validate};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::Object;
use veyndo_app_platform::store::extension_filter::admit;
use veyndo_app_platform::store::extension_projection::descriptions;
use veyndo_cognition::inference::InferenceError;
use veyndo_core::store::lease::Lease;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};
use veyndo_identity::{allowed, merchant, permit};
use veyndo_intelligence::service::models::Models;
/// A grid filter request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    entity: String,
    instruction: String,
    locale: String,
}
/// Whether the caller may plan a filter for any list.
pub fn visible(h: &RequestContext) -> bool {
    allowed(h, "assistant.use")
        && (allowed(h, "catalog.read")
            || (allowed(h, "customers.pii")
                && (allowed(h, "orders.read") || allowed(h, "customers.read"))))
}
/// Plans native list criteria from an instruction; only validated, admitted filters return.
pub async fn plan(m: Models<'_>, h: &RequestContext, v: Value) -> Result<Value> {
    let a = m.core;
    let t = merchant(h)?;
    permit(h, "assistant.use")?;
    let input: Input = serde_json::from_value(v).map_err(|_| bad("Invalid grid request"))?;
    let scope = match input.entity.as_str() {
        "orders" => "orders.read",
        "customers" => "customers.read",
        "products" => "catalog.read",
        _ => return Err(bad("Unknown grid entity")),
    };
    permit(h, scope)?;
    if input.entity != "products" {
        permit(h, "customers.pii")?;
    }
    if input.instruction.trim().is_empty()
        || input.instruction.len() > 2000
        || input.locale.len() > 35
    {
        return Err(bad("Invalid grid instruction"));
    }
    let criteria = criteria_schema(&input.entity)?;
    let schema = json!({"type":"object","properties":{"supported":{"type":"boolean"},"criteria":criteria},"required":["supported","criteria"],"additionalProperties":false});
    let _slot = m.slots.clone().try_acquire_owned().map_err(|_| {
        Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Inference capacity busy; try again shortly".into(),
        )
    })?;
    let _lease = Lease::acquire(a, &t, "model", 2).await?;
    let object = match input.entity.as_str() {
        "products" => Object::Product,
        "customers" => Object::Customer,
        _ => Object::Order,
    };
    let fields = descriptions(a, h, object).await?;
    let prompt=json!({"entity":input.entity,"interfaceLanguage":input.locale,"currentDate":chrono::Utc::now().date_naive().to_string(),"registeredAppFields":fields,"userRequest":input.instruction}).to_string();
    let output = m
        .inference
        .structured_for(
            "planner",
            None,
            include_str!("../../../../crates/cognition/prompts/grid-filter.v1.txt"),
            &prompt,
            &schema,
        )
        .await
        .map_err(|e| match e {
            InferenceError::Admission(_) => Error::from(e),
            _ => Error(
                StatusCode::BAD_GATEWAY,
                "Grid model service unavailable; use manual filters or retry".into(),
            ),
        })?;
    let supported = output.value["supported"]
        .as_bool()
        .ok_or(bad("Invalid grid model result"))?;
    let criteria = validate(&input.entity, &output.value["criteria"])?;
    if let Some(filter) = criteria["appFilter"].as_str() {
        admit(a, h, object, filter).await?;
    }
    if supported && criteria.as_object().unwrap().is_empty() {
        return Err(bad("Grid model returned no filters"));
    }
    Ok(
        json!({"supported":supported,"criteria":criteria,"model":output.model,"provider":output.provider,"promptId":"grid-filter@1"}),
    )
}
