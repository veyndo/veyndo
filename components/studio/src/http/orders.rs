//! Merchant order search with admitted app filters, token-redacted detail with the effective workflow, and append-only operational notes.
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_app_model::Object;
use veyndo_app_platform::domain::native_lifecycle::order_machine;
use veyndo_app_platform::store::extension_filter::{admit as admit_filters, predicate};
use veyndo_app_platform::store::extension_projection::attach;
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{merchant, permit};
use veyndo_orders::domain::order_fields::order_fields;
use veyndo_orders::domain::order_workflow::workflow;
use veyndo_orders::store::machine::machine;
use veyndo_orders::store::{order_list, order_reads};
use veyndo_orders::types::list_criteria::Criteria;

/// The native bounds of an order or customer list plus the admission of app-contributed filters.
pub fn validate_criteria(c: &Criteria) -> Result<i64> {
    let limit = c.validate()?;
    veyndo_app_platform::domain::extension_filter::decode(&c.app_filter)?;
    Ok(limit)
}
/// One page of orders.
pub async fn list(
    Shared(a): Shared<State>,
    h: RequestContext,
    Query(c): Query<Criteria>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "orders.read")?;
    permit(&h, "customers.pii")?;
    let n = validate_criteria(&c)?;
    let filters = admit_filters(&a, &h, Object::Order, &c.app_filter).await?;
    let rows = order_list::rows(&a, &t, &c, n, &|q| {
        predicate(q, Object::Order, &t, &filters)
    })
    .await?;
    let more = rows.len() > n as usize;
    let mut elements = rows
        .iter()
        .take(n as usize)
        .map(|r| {
            let mut o: Value = r.get("data");
            order_fields(&mut o);
            o["createdAt"] = json!(r.get::<String, _>("created"));
            o["customerEmail"] = json!(r.get::<Option<String>, _>("email"));
            o
        })
        .collect::<Vec<_>>();
    attach(&a, &h, Object::Order, &mut elements).await?;
    let after = elements.last().map(|v| v["id"].clone());
    Ok(Json(
        json!({"elements":elements,"hasMore":more,"nextCursor":if more{after}else{None}}),
    ))
}
/// One order with its workflow and app extensions.
pub async fn detail(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "orders.read")?;
    permit(&h, "customers.pii")?;
    let mut v = order_reads::detail(&a, &t, &id).await?;
    let (machine, revision) = machine(&a.db, &t).await?;
    let (machine, revision) = order_machine(&v, machine, revision)?;
    v["workflow"] = workflow(&v, &machine, revision, &h);
    attach(&a, &h, Object::Order, std::slice::from_mut(&mut v)).await?;
    Ok(Json(v))
}
/// Appends an operational note.
pub async fn note(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "orders.write")?;
    permit(&h, "customers.pii")?;
    order_reads::note(&a, &h, &t, &id, &v).await?;
    Ok(Json(json!({"saved":true})))
}
