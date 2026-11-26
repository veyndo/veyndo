//! Project the native checkout and current purchasing constraints into UCP; never authorize a purchase.
use crate::ucp::messages;
use crate::ucp::ucp_meta;
use crate::ucp::views::{BuyerContact, OrderSnapshot, PricedCart, PricedLine};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use veyndo_core::document::view;
use veyndo_shop::types::cart::StoredCart;

/// One advertised capability version, with its optional specification links.
#[derive(Serialize, Clone, Debug, Default)]
pub(crate) struct UcpCapability {
    pub version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<&'static str>,
}

impl UcpCapability {
    pub fn version(version: &'static str) -> Vec<Self> {
        vec![Self {
            version,
            ..Self::default()
        }]
    }
}

/// A transport binding of the shopping service in the discovery profile.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UcpService {
    pub version: &'static str,
    pub spec: &'static str,
    pub transport: &'static str,
    pub schema: &'static str,
    pub endpoint: String,
}

/// The `ucp` metadata block of a profile or document.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UcpMeta {
    pub version: &'static str,
    pub capabilities: BTreeMap<&'static str, Vec<UcpCapability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<BTreeMap<&'static str, Vec<UcpService>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_handlers: Option<BTreeMap<String, Value>>,
}

/// A UCP message.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UcpMessage {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub code: &'static str,
    pub content: &'static str,
}

impl UcpMessage {
    pub fn info(code: &'static str, locale: &str) -> Self {
        Self {
            kind: "info",
            code,
            content: messages::content(code, locale),
        }
    }
}

/// An amount in integer minor units.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UcpTotal {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub amount: i64,
}

/// The item a line refers to.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UcpItem {
    pub id: Value,
    pub title: Value,
    pub price: i64,
}

/// One UCP line item.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UcpLineItem {
    pub id: Value,
    pub item: UcpItem,
    pub quantity: Value,
    pub totals: Vec<UcpTotal>,
}

/// Payment instruments; the adapter never stores any.
#[derive(Serialize, Clone, Debug, Default)]
pub(crate) struct UcpPayment {
    pub instruments: Vec<Value>,
}

/// The completed order reference.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UcpOrderLink {
    pub id: Value,
    pub permalink_url: Option<String>,
}

/// The Veyndo B2B negotiation extension block.
#[derive(Serialize, Clone, Debug, Default)]
pub(crate) struct UcpNegotiation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout_revision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchasing: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quotes: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_limit: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

/// A UCP checkout (or cart) document. `buyer` is the client's open object passed through with the projected consent.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct UcpDocument {
    pub ucp: UcpMeta,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'static str>,
    pub currency: Value,
    pub line_items: Vec<UcpLineItem>,
    pub totals: Vec<UcpTotal>,
    pub messages: Vec<UcpMessage>,
    pub links: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment: Option<UcpPayment>,
    pub continue_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<UcpOrderLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b2b_negotiation: Option<UcpNegotiation>,
}

impl UcpDocument {
    /// The wire JSON of the document.
    pub fn json(&self) -> Value {
        serde_json::to_value(self).expect("UCP documents serialize")
    }
}

pub(crate) fn ucp_document(c: &StoredCart, q: &Value) -> UcpDocument {
    let quote: PricedCart = view(q);
    let factor = 10_f64.powi(quote.price.currency_scale.unwrap_or(2) as i32);
    let minor = |v: Option<f64>| (v.unwrap_or(0.) * factor).round() as i64;
    let status = if c.status == "completed" {
        "completed"
    } else if c.status == "cancelled" {
        "canceled"
    } else if BuyerContact::has_email(c.data.buyer.as_ref()) && !c.data.items.is_empty() {
        "ready_for_complete"
    } else {
        "incomplete"
    };
    let line_items = quote
        .line_items
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            let l: PricedLine = view(l);
            let total = minor(l.price.total_price);
            UcpLineItem {
                id: l.id,
                item: UcpItem {
                    id: l.referenced_id,
                    title: l.label,
                    price: minor(l.price.unit_price),
                },
                quantity: l.quantity,
                totals: vec![
                    UcpTotal {
                        kind: "subtotal",
                        amount: total,
                    },
                    UcpTotal {
                        kind: "total",
                        amount: total,
                    },
                ],
            }
        })
        .collect();
    let order = c.data.order.as_ref().map(view::<OrderSnapshot>);
    let mut doc = UcpDocument {
        ucp: ucp_meta(),
        id: c.id.clone(),
        status: Some(status),
        currency: quote.price.currency,
        line_items,
        totals: vec![
            UcpTotal {
                kind: "subtotal",
                amount: minor(quote.price.position_price),
            },
            UcpTotal {
                kind: "total",
                amount: minor(quote.price.total_price),
            },
        ],
        messages: if c.status == "open" {
            vec![UcpMessage::info("requires_buyer_input", &c.data.locale)]
        } else {
            vec![]
        },
        links: vec![],
        payment: Some(UcpPayment::default()),
        continue_url: None,
        order: order.as_ref().map(|o| UcpOrderLink {
            id: o.id.clone(),
            permalink_url: None,
        }),
        buyer: c.data.buyer.clone(),
        context_token: None,
        expires_at: None,
        b2b_negotiation: None,
    };
    if let Some(o) = &order
        && o.payment.provider.as_deref() == Some("paypal")
        && !["captured", "partially_refunded", "refunded"]
            .contains(&o.payment.state.as_deref().unwrap_or(""))
    {
        doc.status = Some(
            if ["cancelled", "expired"].contains(&o.payment.state.as_deref().unwrap_or("")) {
                "canceled"
            } else {
                "requires_escalation"
            },
        );
        doc.messages = vec![UcpMessage::info(
            "payment_customer_action_required",
            &c.data.locale,
        )];
    }
    doc
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;
    use veyndo_shop::types::cart::CartRow;

    /// A stored cart with the given status, buyer and order snapshot.
    pub(crate) fn cart(status: &str, buyer: Value, order: Value) -> StoredCart {
        CartRow {
            id: "cart-1".into(),
            tenant: "shop".into(),
            token: "token".into(),
            data: json!({"items":[{"id":"p1","quantity":2}],"group":"default","email":null,"company":null,
                "session":"s","buyer":buyer,"order":order,"locale":"de-DE"}),
            revision: 4,
            status: status.into(),
        }
        .hydrate()
        .unwrap()
    }

    /// The projection as it was built from indexed JSON before the typed document.
    fn legacy(c: &StoredCart, q: &Value) -> Value {
        let factor = 10_f64.powi(q["price"]["currencyScale"].as_i64().unwrap_or(2) as i32);
        let minor = |v: &Value| (v.as_f64().unwrap_or(0.) * factor).round() as i64;
        let status = if c.status == "completed" {
            "completed"
        } else if c.status == "cancelled" {
            "canceled"
        } else if c
            .data
            .buyer
            .as_ref()
            .and_then(|b| b["email"].as_str())
            .is_some_and(|s| s.contains('@'))
            && !c.data.items.is_empty()
        {
            "ready_for_complete"
        } else {
            "incomplete"
        };
        let mut doc = json!({"ucp":{"version":crate::ucp::UCP_VERSION,"capabilities":{"dev.ucp.shopping.checkout":[{"version":crate::ucp::UCP_VERSION}]}},"id":c.id,"status":status,"currency":q["price"]["currency"],"line_items":q["lineItems"].as_array().unwrap().iter().map(|l|json!({"id":l["id"],"item":{"id":l["referencedId"],"title":l["label"],"price":minor(&l["price"]["unitPrice"])},"quantity":l["quantity"],"totals":[{"type":"subtotal","amount":minor(&l["price"]["totalPrice"])},{"type":"total","amount":minor(&l["price"]["totalPrice"])}]})).collect::<Vec<_>>(),"totals":[{"type":"subtotal","amount":minor(&q["price"]["positionPrice"])},{"type":"total","amount":minor(&q["price"]["totalPrice"])}],"messages":if c.status=="open"{json!([{"type":"info","code":"requires_buyer_input","content":messages::content("requires_buyer_input",&c.data.locale)}])}else{json!([])},"links":[],"payment":{"instruments":[]},"continue_url":null,"order":c.data.order.as_ref().map(|o|json!({"id":o["id"],"permalink_url":null}))});
        if let Some(o) = &c.data.order
            && o["payment"]["provider"] == "paypal"
            && !["captured", "partially_refunded", "refunded"]
                .contains(&o["payment"]["state"].as_str().unwrap_or(""))
        {
            doc["status"] = json!(if ["cancelled", "expired"]
                .contains(&o["payment"]["state"].as_str().unwrap_or(""))
            {
                "canceled"
            } else {
                "requires_escalation"
            });
            doc["messages"] = json!([{"type":"info","code":"payment_customer_action_required","content":messages::content("payment_customer_action_required",&c.data.locale)}]);
        }
        if c.data.order.is_none() {
            doc.as_object_mut().unwrap().remove("order");
        }
        if let Some(b) = &c.data.buyer {
            doc["buyer"] = b.clone();
        }
        doc
    }

    #[test]
    fn typed_document_matches_the_indexed_projection() {
        let quotes = [
            json!({"price":{"currencyScale":2,"currency":"EUR","positionPrice":10.5,"totalPrice":12.495},
                "lineItems":[{"id":"l1","referencedId":"p1","label":"Shirt","quantity":2,"price":{"unitPrice":5.25,"totalPrice":10.5}},
                    {"id":"l2","label":7,"price":"bad"}, 3]}),
            json!({"price":{"currencyScale":0,"currency":null,"positionPrice":"1","totalPrice":3},"lineItems":[]}),
            json!({"price":"none","lineItems":[]}),
        ];
        let carts = [
            cart("open", json!(null), json!(null)),
            cart("open", json!({"email":"a@b.c","name":"A"}), json!(null)),
            cart("completed", json!({"email":"x"}), json!({"id":"o1"})),
            cart(
                "completed",
                json!(null),
                json!({"id":"o2","payment":{"provider":"paypal","state":"pending"}}),
            ),
            cart(
                "completed",
                json!(null),
                json!({"id":"o3","payment":{"provider":"paypal","state":"expired"}}),
            ),
            cart(
                "cancelled",
                json!(null),
                json!({"id":4,"payment":{"provider":"paypal","state":"captured"}}),
            ),
            cart("open", json!(null), json!("odd")),
        ];
        for c in &carts {
            for q in &quotes {
                assert_eq!(ucp_document(c, q).json(), legacy(c, q));
            }
        }
    }
}
