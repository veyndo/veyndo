//! Customer-owned order reads share one tenant/identity predicate and a public projection.
use crate::domain::order_fields::order_fields;
use crate::store::account_orders;
use crate::store::machine::machine;
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_app_model::native_lifecycle_snapshot::project_labels;
use veyndo_core::shop_scope::{session_digest, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_customers::service::sessions::identify;

/// The signed-in customer of the request and their shop.
async fn customer(a: &State, h: &RequestContext) -> Result<(String, String)> {
    let t = tenant(h)?;
    let email = identify(a, &t, &session_digest(h)?).await?;
    Ok((t, email))
}

/// The customer's orders in the public projection; an account ID snapshot owns modern orders, a matching guest email never grants access.
pub async fn owned(
    a: &State,
    t: &str,
    email: &str,
    id: Option<&str>,
    after: Option<&str>,
) -> Result<Vec<Value>> {
    Ok(account_orders::owned(a, t, email, id, after)
        .await?
        .iter()
        .map(|(data, time)| project(data, time))
        .collect())
}
/// The public projection of an order: no operational notes, activity, cart token or payment attempt.
pub fn project(raw: &Value, created: &str) -> Value {
    let mut v = json!({});
    for key in [
        "id",
        "orderNumber",
        "revision",
        "state",
        "orderDateTime",
        "billingAddress",
        "shippingAddress",
        "billingAddressId",
        "shippingAddressId",
        "deliveries",
        "legal",
    ] {
        v[key] = raw[key].clone();
    }
    v["cart"] = json!({"id":raw["cart"]["id"], "price":raw["cart"]["price"], "shippingCosts":raw["cart"]["shippingCosts"], "lineItems":raw["cart"]["lineItems"]});
    v["payment"] = json!({"state":raw["payment"]["state"], "provider":raw["payment"]["provider"], "method":raw["payment"]["method"]});
    v["createdAt"] = json!(created);
    order_fields(&mut v);
    project_labels(raw, &mut v);
    v
}
/// The cursor of the order list.
#[derive(Default, Deserialize)]
pub struct Page {
    after: Option<String>,
}
/// `GET /store-api/account/orders`: a page of the customer's orders with their state labels.
pub async fn list(
    Shared(a): Shared<State>,
    h: RequestContext,
    Query(page): Query<Page>,
) -> Result<Json<Value>> {
    let (t, email) = customer(&a, &h).await?;
    if let Some(after) = &page.after
        && (after.len() > 128 || owned(&a, &t, &email, Some(after), None).await?.is_empty())
    {
        return Err(Error(StatusCode::NOT_FOUND, "Order not found".into()));
    }
    let mut elements = owned(&a, &t, &email, None, page.after.as_deref()).await?;
    let has_more = elements.len() > 100;
    elements.truncate(100);
    let next = if has_more {
        elements.last().map(|v| v["id"].clone())
    } else {
        None
    };
    let (machine, _) = machine(&a.db, &t).await?;
    let labels = json!(
        machine
            .states
            .iter()
            .map(|s| (&s.id, &s.label))
            .collect::<HashMap<_, _>>()
    );
    for v in &mut elements {
        if !v["stateLabels"].is_object() {
            v["stateLabels"] = labels.clone();
        }
    }
    Ok(Json(json!({"elements":elements,"nextCursor":next})))
}
/// `GET /store-api/account/orders/{id}`: one order with its receipts and state labels.
pub async fn detail(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let (t, email) = customer(&a, &h).await?;
    let mut v = owned(&a, &t, &email, Some(&id), None)
        .await?
        .pop()
        .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
    v["receipts"] = json!(account_orders::receipts(&a, &t, &id).await?);
    let (machine, _) = machine(&a.db, &t).await?;
    if !v["stateLabels"].is_object() {
        v["stateLabels"] = json!(
            machine
                .states
                .iter()
                .map(|s| (&s.id, &s.label))
                .collect::<HashMap<_, _>>()
        );
    }
    Ok(Json(v))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_does_not_leak_operational_or_session_data() {
        let raw = json!({"id":"own", "automation":{"notes":["Internal"]}, "activity":[{"actor":"private"}], "cart":{"token":"secret", "checkout":{"customerToken":"secret"}, "price":{"totalPrice":12}}, "payment":{"state":"paid","provider":"manual","attemptId":"private"}});
        let v = project(&raw, "2026-10-06");
        assert_eq!(v["id"], "own");
        assert_eq!(v["amountTotal"], 12);
        assert!(v.get("automation").is_none());
        assert!(v.get("activity").is_none());
        assert!(v["cart"].get("token").is_none());
        assert!(v["cart"].get("checkout").is_none());
        assert!(v["payment"].get("attemptId").is_none());
    }
}
