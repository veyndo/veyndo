//! Tenant-scoped paged CRM with admitted app filters and revision-checked merchant changes; credentials never leave storage.
use super::orders::validate_criteria;
use crate::store::customer_directory as directory;
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde_json::{Value, json};
use veyndo_app_model::Object;
use veyndo_app_platform::store::extension_filter::admit as admit_filters;
use veyndo_app_platform::store::extension_projection::attach;
use veyndo_core::{RequestContext, Result, State};
use veyndo_customers::service::address_entries::address_list;
use veyndo_customers::service::directory::{decorate, guest};
use veyndo_identity::{merchant, permit};
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_orders::types::list_criteria::Criteria;
/// One page of the CRM.
pub async fn list(
    Shared(a): Shared<State>,
    h: RequestContext,
    Query(c): Query<Criteria>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.read")?;
    permit(&h, "customers.pii")?;
    let n = validate_criteria(&c)?;
    let (sort, direction) = c.sort_for("customers")?;
    let filters = admit_filters(&a, &h, Object::Customer, &c.app_filter).await?;
    let rows = directory::page(&a, &t, c, (sort, direction), &filters, n).await?;
    let more = rows.len() > n as usize;
    let mut elements = rows.into_iter().take(n as usize).collect::<Vec<_>>();
    attach(&a, &h, Object::Customer, &mut elements).await?;
    let after = elements.last().map(|v| v["email"].clone());
    Ok(Json(
        json!({"elements":elements,"hasMore":more,"nextCursor":if more{after}else{None}}),
    ))
}
/// The native customer projection of a row.
fn value(r: &sqlx::postgres::PgRow) -> Value {
    json!(veyndo_customers::store::customers::customer(r))
}
/// One customer with addresses, orders and app extensions.
pub async fn detail(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(email): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.read")?;
    permit(&h, "customers.pii")?;
    let r = directory::customer(&a, &t, &email).await?;
    let mut v = if let Some(r) = r {
        let mut v = value(&r);
        decorate(&a, &t, &email, &mut v).await?;
        v["addresses"] = address_list(&a, &t, &email).await?;
        v
    } else {
        guest(&a, &t, &email).await?
    };
    if permit(&h, "orders.read").is_ok() {
        v["orders"] = json!(directory::orders(&a, &t, &email).await?);
    }
    attach(&a, &h, Object::Customer, std::slice::from_mut(&mut v)).await?;
    Ok(Json(v))
}
/// Saves a customer at its revision.
pub async fn save(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(email): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    save_domain(a, h, email, v, None).await
}
/// Restores a customer from a history entry through the same revision-checked save.
pub async fn restore_customer(
    a: State,
    h: RequestContext,
    email: String,
    state: Value,
    revision: i64,
) -> Result<Json<Value>> {
    let value = json!({"revision":revision,"profile":state["profile"],"company":state["company"],"customerGroup":state["customerGroup"],"active":state["active"]});
    save_domain(a, h, email, value, Some(state)).await
}
/// The shared merchant save of a customer.
async fn save_domain(
    a: State,
    h: RequestContext,
    email: String,
    v: Value,
    book: Option<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.write")?;
    permit(&h, "customers.pii")?;
    Ok(Json(
        veyndo_customers::store::crm::save(&a, &h, &t, &email, v, book, &validate_condition)
            .await?,
    ))
}
