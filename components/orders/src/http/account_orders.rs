//! Customer-owned order reads share one tenant/identity predicate and a public projection.
use crate::domain::order_fields::order_fields;
use crate::domain::order_patches::CreatedAt;
use crate::store::account_orders;
use crate::store::machine::machine;
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_app_model::native_lifecycle_snapshot::project_labels;
use veyndo_core::document::{lenient, merge, view};
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
/// The order a customer may see: a closed whitelist, never operational notes, activity or session data.
#[derive(Deserialize, Serialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CustomerOrder {
    id: Value,
    order_number: Value,
    revision: Value,
    state: Value,
    order_date_time: Value,
    billing_address: Value,
    shipping_address: Value,
    billing_address_id: Value,
    shipping_address_id: Value,
    deliveries: Value,
    legal: Value,
    #[serde(deserialize_with = "lenient")]
    cart: CustomerCart,
    #[serde(deserialize_with = "lenient")]
    payment: CustomerPayment,
}

#[derive(Deserialize, Serialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CustomerCart {
    id: Value,
    price: Value,
    shipping_costs: Value,
    line_items: Value,
}

#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
struct CustomerPayment {
    state: Value,
    provider: Value,
    method: Value,
}

/// The labels of the order machine states, as the account projection shows them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateLabels<'a> {
    state_labels: &'a Value,
}

/// The customer-visible receipts of an order.
#[derive(Serialize)]
struct OrderReceipts {
    receipts: Vec<Value>,
}

pub fn project(raw: &Value, created: &str) -> Value {
    let order: CustomerOrder = view(raw);
    let mut v = json!(order);
    // A serialized record is an object, so the merge cannot fail.
    let _ = merge(
        &mut v,
        CreatedAt {
            created_at: &created,
        },
    );
    order_fields(&mut v);
    project_labels(raw, &mut v);
    v
}
/// The fields of a projected order the list and detail read back.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Listed {
    id: Value,
    state_labels: Value,
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
        elements.last().map(|v| view::<Listed>(v).id)
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
        if !view::<Listed>(v).state_labels.is_object() {
            merge(
                v,
                StateLabels {
                    state_labels: &labels,
                },
            )?;
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
    merge(
        &mut v,
        OrderReceipts {
            receipts: account_orders::receipts(&a, &t, &id).await?,
        },
    )?;
    let (machine, _) = machine(&a.db, &t).await?;
    if !view::<Listed>(&v).state_labels.is_object() {
        let labels = json!(
            machine
                .states
                .iter()
                .map(|s| (&s.id, &s.label))
                .collect::<HashMap<_, _>>()
        );
        merge(
            &mut v,
            StateLabels {
                state_labels: &labels,
            },
        )?;
    }
    Ok(Json(v))
}
#[cfg(test)]
mod tests {
    /// The typed account projection patches write exactly what the indexed assignments wrote.
    #[test]
    fn typed_account_patches_match_the_indexed_writes() {
        let labels = json!({"placed":{"en":"Placed"}});
        let mut indexed = json!({"id":"o1"});
        indexed["stateLabels"] = labels.clone();
        indexed["receipts"] = json!([{"id":"r1"}]);
        let mut typed = json!({"id":"o1"});
        merge(
            &mut typed,
            StateLabels {
                state_labels: &labels,
            },
        )
        .unwrap();
        merge(
            &mut typed,
            OrderReceipts {
                receipts: vec![json!({"id":"r1"})],
            },
        )
        .unwrap();
        assert_eq!(typed, indexed);
    }

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
