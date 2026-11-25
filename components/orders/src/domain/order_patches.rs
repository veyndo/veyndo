//! Typed patches of order documents shared by order transitions, splits, documents, receipts and the payment ledger: each writes exactly the fields indexed assignment wrote.
use serde::Serialize;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::document_slot::field;

/// Removes the buyer's cart token from a document's `cart`; a missing cart becomes null, exactly as the indexed removal left it.
pub fn strip_cart_token(doc: &mut Value) -> Result<()> {
    if let Some(cart) = field(doc, "cart")?.as_object_mut() {
        cart.remove("token");
    }
    Ok(())
}

/// The state of an order, its payment or one delivery.
#[derive(Serialize)]
pub struct Stated<'a> {
    pub state: &'a str,
}

/// The order revision after a change.
#[derive(Serialize)]
pub struct Revised<T: Serialize> {
    pub revision: T,
}

/// The lifecycle workflow a merchant response carries.
#[derive(Serialize)]
pub struct Workflowed {
    pub workflow: Value,
}

/// A delivery's tracking code.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingCode<'a> {
    pub tracking_code: &'a str,
}

/// A delivery's tracking link; empty clears it to null.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingUrl<'a> {
    pub tracking_url: Option<&'a str>,
}

impl<'a> TrackingUrl<'a> {
    /// The link, or null for an empty one.
    pub fn of(url: &'a str) -> Self {
        TrackingUrl {
            tracking_url: (!url.is_empty()).then_some(url),
        }
    }
}

/// When an order was created, as projections add it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedAt<'a, T: Serialize> {
    pub created_at: &'a T,
}

#[cfg(test)]
mod tests {
    use super::{
        CreatedAt, Revised, Stated, TrackingCode, TrackingUrl, Workflowed, strip_cart_token,
    };
    use serde_json::{Value, json};
    use veyndo_core::document::merge;
    use veyndo_core::document_slot::field;

    /// Each shared patch writes exactly the document the indexed assignment wrote.
    #[test]
    fn shared_order_patches_match_the_indexed_writes() {
        for base in [
            json!({"id":"o1","cart":{"id":"c","token":"secret"},"payment":{"state":"pending"},"deliveries":[{"state":"open"}]}),
            json!({"id":"o1"}),
            Value::Null,
        ] {
            let mut indexed = base.clone();
            if let Some(cart) = indexed["cart"].as_object_mut() {
                cart.remove("token");
            }
            indexed["payment"]["state"] = json!("cancelled");
            indexed["state"] = json!("cancelled");
            indexed["revision"] = json!(4);
            indexed["workflow"] = json!({"next":[]});
            indexed["createdAt"] = json!("2026-01-01");
            let mut typed = base.clone();
            strip_cart_token(&mut typed).unwrap();
            merge(
                field(&mut typed, "payment").unwrap(),
                Stated { state: "cancelled" },
            )
            .unwrap();
            merge(&mut typed, Stated { state: "cancelled" }).unwrap();
            merge(&mut typed, Revised { revision: 4_i64 }).unwrap();
            merge(
                &mut typed,
                Workflowed {
                    workflow: json!({"next":[]}),
                },
            )
            .unwrap();
            merge(
                &mut typed,
                CreatedAt {
                    created_at: &"2026-01-01",
                },
            )
            .unwrap();
            assert_eq!(typed, indexed);
        }
        for url in ["", "https://track.example/1"] {
            let mut indexed = json!({"state":"open"});
            indexed["trackingCode"] = json!("T1");
            indexed["trackingUrl"] = if url.is_empty() {
                Value::Null
            } else {
                json!(url)
            };
            let mut typed = json!({"state":"open"});
            merge(
                &mut typed,
                TrackingCode {
                    tracking_code: "T1",
                },
            )
            .unwrap();
            merge(&mut typed, TrackingUrl::of(url)).unwrap();
            assert_eq!(typed, indexed);
        }
    }
}
