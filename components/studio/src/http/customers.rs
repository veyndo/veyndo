//! Tenant-scoped paged CRM with admitted app filters and revision-checked merchant changes; credentials never leave storage.
use super::orders::validate_criteria;
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_app_model::Object;
use veyndo_app_platform::store::extension_filter::{admit as admit_filters, predicate};
use veyndo_app_platform::store::extension_projection::attach;
use veyndo_core::{RequestContext, Result, State, bad};
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
    let rows = if filters.is_empty()
        && (c.sort.is_empty() || c.sort == "email")
        && direction == "ASC"
        && c.guest.is_none()
        && c.active.is_none()
        && c.group.is_empty()
    {
        sqlx::query_scalar::<_, Value>(include_str!("../store/customer_directory_default.sql"))
            .bind(&t)
            .bind(c.after)
            .bind(c.query)
            .bind(n + 1)
            .fetch_all(&a.db)
            .await?
    } else {
        use sqlx::Arguments;
        let mut arguments = sqlx::postgres::PgArguments::default();
        arguments
            .add(t.clone())
            .map_err(|_| bad("Invalid tenant binding"))?;
        let mut q = sqlx::QueryBuilder::<sqlx::Postgres>::with_arguments(
            include_str!("../store/customer_directory.sql"),
            arguments,
        );
        if !c.after.is_empty() {
            q.push(format!(
                " AND ({sort},email) {} (SELECT {sort},email FROM contacts WHERE email=",
                if direction == "ASC" { ">" } else { "<" }
            ))
            .push_bind(c.after)
            .push(")");
        }
        if !c.query.is_empty() {
            q.push(" AND strpos(lower(email||' '||coalesce(contact->'profile'->>'name','')||' '||coalesce(contact->>'company','')),lower(").push_bind(c.query).push("))>0");
        }
        if let Some(guest) = c.guest {
            q.push(" AND (contact->>'guest')::bool=").push_bind(guest);
        }
        if let Some(active) = c.active {
            q.push(" AND (contact->>'active')::bool=").push_bind(active);
        }
        if !c.group.is_empty() {
            q.push(" AND contact->>'customerGroup'=").push_bind(c.group);
        }
        predicate(&mut q, Object::Customer, &t, &filters);
        q.push(format!(
            " ORDER BY {sort} {direction},email {direction} LIMIT "
        ))
        .push_bind(n + 1);
        q.build_query_scalar::<Value>().fetch_all(&a.db).await?
    };
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
    let r = sqlx::query(
        "SELECT *,created_at::text AS created FROM customers WHERE tenant=$1 AND email=$2",
    )
    .bind(&t)
    .bind(&email)
    .fetch_optional(&a.db)
    .await?;
    let mut v = if let Some(r) = r {
        let mut v = value(&r);
        decorate(&a, &t, &email, &mut v).await?;
        v["addresses"] = address_list(&a, &t, &email).await?;
        v
    } else {
        guest(&a, &t, &email).await?
    };
    if permit(&h, "orders.read").is_ok() {
        let rows=sqlx::query("SELECT o.data #- '{cart,token}' AS data,o.created_at::text AS created FROM orders o JOIN carts c ON c.id=o.cart_id AND c.tenant=o.tenant WHERE o.tenant=$1 AND ((o.data->'orderCustomer'->>'customerId')=(SELECT id FROM customers WHERE tenant=o.tenant AND email=$2) OR (o.data->'orderCustomer'->>'guest'='true' AND o.data->'orderCustomer'->>'email'=$2) OR (NOT o.data ? 'orderCustomer' AND c.data->>'email'=$2)) ORDER BY o.created_at DESC LIMIT 100").bind(t).bind(email).fetch_all(&a.db).await?;
        v["orders"] = json!(
            rows.iter()
                .map(|r| {
                    let mut o: Value = r.get("data");
                    o["createdAt"] = json!(r.get::<String, _>("created"));
                    o
                })
                .collect::<Vec<_>>()
        );
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
