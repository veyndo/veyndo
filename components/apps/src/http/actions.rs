//! The shared action adapter, generated data endpoints and storefront slots of installed apps.
use crate::domain::entity_access::entity_access;
use crate::ports::Platform;
use crate::service::gateway;
use crate::store::managed_data;
use crate::store::packages::{self, package};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::shop_scope::tenant;
use veyndo_core::{RequestContext, Result, bad};
use veyndo_identity::{merchant, permit};

/// Runs one app action for the caller.
pub async fn app_action(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path((id, action)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(gateway::invoke(&a, &h, &id, &action, &v, None).await?))
}
/// One page of the records of an entity for a merchant editor.
pub async fn entity_list(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path((id, name)): Path<(String, String)>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let m = package(&a, &t, &id, true).await?;
    let e = m
        .entities
        .iter()
        .find(|e| e.name == name)
        .ok_or(bad("Unknown entity"))?;
    entity_access(&m, e, &h, "list")?;
    let input = json!({"limit":q.get("limit").map(|s| s.parse::<i64>()).transpose().map_err(|_| bad("Invalid app limit"))?,"after":q.get("after"),"filter":q.get("filter").map(|s| serde_json::from_str::<Value>(s)).transpose().map_err(|_| bad("Invalid app filter"))?});
    let mut input = input;
    if input["filter"].is_null() {
        input.as_object_mut().unwrap().remove("filter");
    }
    Ok(Json(managed_data::list_page(&a, &t, &m, e, &input).await?))
}
/// Saves one record of an entity from a merchant editor.
pub async fn entity_save(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path((id, name)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    permit(&h, "catalog")?;
    let t = merchant(&h)?;
    let m = package(&a, &t, &id, true).await?;
    let e = m
        .entities
        .iter()
        .find(|e| e.name == name)
        .ok_or(bad("Unknown entity"))?;
    entity_access(&m, e, &h, "save")?;
    Ok(Json(managed_data::save(&a, &t, &m, e, &v, &h).await?))
}
/// The storefront slots of the active packages with their public models.
pub async fn slots(Shared(a): Shared<Platform>, h: RequestContext) -> Result<Json<Value>> {
    let rows = packages::active_manifest_values(&a, &tenant(&h)?).await?;
    let mut slots = vec![];
    for r in rows {
        let m: Manifest = serde_json::from_value(r).map_err(|_| bad("Invalid package"))?;
        if m.permissions.contains(&"storefront.slot".into()) {
            for s in m.slots.iter().filter(|s| s.location == "product.detail") {
                slots.push(json!({"app":m.id,"version":m.version,"slot":s,"entities":m.entities.iter().filter(|e|e.public_read).map(|e|json!({"name":e.name,"label":e.label,"fields":e.fields,"action":m.actions.iter().find(|a|a.public && a.handler=="list" && a.entity.as_deref()==Some(e.name.as_str())).map(|a|&a.name)})).collect::<Vec<_>>(),"configuration":m.configuration.as_ref().map(|c|json!({"inputField":c.input_field,"label":c.label,"hint":c.hint,"inputs":c.inputs.as_ref().map(|i|json!({"fields":i.fields,"arguments":i.arguments,"recordField":i.record_field})),"entity":c.entity,"currency":c.currency}))}));
            }
        }
    }
    Ok(Json(json!({"slots":slots})))
}
