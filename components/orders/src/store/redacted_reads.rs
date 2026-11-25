//! Explicit non-PII domain read views for separately scoped app callbacks; never elevate an identity.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
use veyndo_customers::store::purchase_customers::redacted;
fn projection(value: &Value, keys: &[&str]) -> Value {
    json!(
        keys.iter()
            .filter_map(|k| value.get(*k).map(|v| ((*k).to_owned(), v.clone())))
            .collect::<serde_json::Map<String, Value>>()
    )
}

pub async fn redacted_read(a: &State, h: &RequestContext, kind: &str, id: &str) -> Result<Value> {
    let tenant = veyndo_identity::merchant(h)?;
    match kind {
        "order" => {
            veyndo_identity::permit(h, "orders.read")?;
            let row = sqlx::query!(
                "SELECT data,created_at::text AS created FROM orders WHERE tenant=$1 AND id=$2",
                tenant,
                id
            )
            .fetch_optional(&a.db)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
            let mut value: Value = row.data;
            crate::domain::order_fields::order_fields(&mut value);
            value["createdAt"] = json!(row.created);
            Ok(order_view(&value))
        }
        "customer" => {
            veyndo_identity::permit(h, "customers.read")?;
            let row = redacted(&a.db, &tenant, id)
                .await?
                .ok_or(Error(StatusCode::NOT_FOUND, "Customer not found".into()))?;
            Ok(
                json!({"id":row.id,"active":row.active,"guest":false,"groupId":row.group_name,"createdAt":row.created}),
            )
        }
        _ => Err(bad("Unknown redacted read")),
    }
}
/// The JSON fields `order_view` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OrderViewInput {
    line_items: Value,
}

fn order_view(value: &Value) -> Value {
    let input: OrderViewInput = view(value);
    let mut clean = projection(
        value,
        &[
            "id",
            "number",
            "orderNumber",
            "state",
            "paymentState",
            "deliveryState",
            "currency",
            "currencyScale",
            "totalMinor",
            "total",
            "createdAt",
        ],
    );
    clean["items"] = json!(
        input
            .line_items
            .as_array()
            .map(|items| items
                .iter()
                .take(100)
                .map(|item| projection(
                    item,
                    &[
                        "referencedId",
                        "quantity",
                        "price",
                        "unitPrice",
                        "totalPrice"
                    ]
                ))
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    clean
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_explicit_fields_survive_nested_personal_data() {
        let value = json!({"id":"order","customerEmail":"PRIVATE", "activity":["PRIVATE"], "cart":{"token":"PRIVATE"},"lineItems":[{"referencedId":"product","quantity":2,"label":"PRIVATE","payload":{"name":"PRIVATE"}}]});
        let clean = order_view(&value);
        assert_eq!(clean["items"][0]["quantity"], 2);
        assert!(!clean.to_string().contains("PRIVATE"));
        assert!(clean.get("cart").is_none());
    }
}
