//! Explicit integration permission imports requisitions into native lists; it never impersonates checkout buyers.
use crate::service::quick_order;
use crate::store::access::{access, require};
use crate::store::lists;
use crate::store::{catalog, organization};
use crate::types::events::RequisitionCreated;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_app_model::declarations::identifier;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad, conflict, store::history_context};
use veyndo_shop::domain::cart_lines;
use veyndo_shop::types::settings::Settings;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    name: HashMap<String, String>,
    lines: Value,
    #[serde(default)]
    currency: Option<String>,
}
/// An imported requisition for one purchasing unit and buyer.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RequisitionImport {
    #[serde(deserialize_with = "lenient")]
    unit_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    customer_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    data: Value,
}

/// `t` is the admitted merchant shop; the list is created once per requisition ID.
pub async fn import(
    a: &State,
    h: &RequestContext,
    t: &str,
    settings: &Settings,
    v: &Value,
) -> Result<Value> {
    let input: RequisitionImport = view(v);
    let unit = input
        .unit_id
        .as_deref()
        .ok_or(bad("Purchasing unit required"))?;
    let buyer = input
        .customer_id
        .as_deref()
        .ok_or(bad("Mapped buyer required"))?;
    let id = input
        .id
        .as_deref()
        .filter(|s| identifier(s))
        .ok_or(bad("Valid requisition ID required"))?;
    let data: Input = serde_json::from_value(input.data.clone())
        .map_err(|_| bad("Expected requisition name and SKU lines"))?;
    organization::name(&data.name, settings)?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "b2b.requisition").await?;
    let policy = access(&mut tx, t, unit, buyer).await?;
    require(&policy, "lists")?;
    require(&policy, "buy")?;
    if data
        .currency
        .as_deref()
        .is_some_and(|c| c != policy.unit.currency)
    {
        return Err(bad("Document currency differs from mapped purchasing unit"));
    }
    let items = quick_order::resolve(&mut tx, t, &data.lines).await?;
    cart_lines::validate(&items)?;
    for item in &items {
        catalog::admit_unit(&mut tx, t, unit, &item.id, &policy).await?;
    }
    lists::serialize(&mut tx, t, id).await?;
    let normalized = json!({"name":data.name,"shared":false,"items":items});
    if let Some(row) = lists::locked(&mut tx, t, id).await? {
        if row.owner_id != buyer || row.unit_id != unit || row.data != normalized {
            return Err(conflict("Requisition ID already has different content"));
        }
        return Ok(json!({"id":id,"revision":row.revision,"replayed":true}));
    }
    let owner = lists::Owner { unit, owner: buyer };
    lists::insert(&mut tx, t, id, owner, &normalized).await?;
    let created = RequisitionCreated {
        id,
        unit_id: unit,
        revision: 1,
    };
    organization::event(&mut tx, t, &created).await?;
    tx.commit().await?;
    Ok(json!({"id":id,"revision":1,"replayed":false}))
}
