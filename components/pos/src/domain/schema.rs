//! Strict protocol schemas for the same POS operations used by HTTP and MCP.
use serde_json::{Value, json};
pub const CAPABILITIES: &[(&str, &str)] = &[
    (
        "pos.customers",
        "Search active tenant customers or resolve a customer-number card",
    ),
    (
        "pos.customer",
        "Revision-checked cashier attachment of an existing customer",
    ),
    (
        "pos.discount",
        "Audited exact cashier discount through shared pricing",
    ),
    (
        "pos.receipt",
        "Read printable lines from an existing immutable issued invoice",
    ),
    (
        "pos.channels",
        "Discover the tenant POS registers through cashier admission",
    ),
    (
        "pos.payment",
        "Read the owned sale payment and authoritative order",
    ),
    (
        "pos.reconcile",
        "Reconcile the owned provider attempt through existing durable jobs",
    ),
    (
        "pos.catalog",
        "Search channel products or resolve a barcode for an authenticated cashier",
    ),
    (
        "pos.context",
        "Read POS configuration without creating a cart",
    ),
    (
        "pos.detail",
        "Read native product families and variant prices",
    ),
    (
        "pos.currency",
        "Select an enabled channel currency through native pricing",
    ),
    (
        "pos.coupons",
        "Apply coupon codes through the native promotion owner",
    ),
    ("pos.create", "Create an actor-bound native POS cart"),
    (
        "pos.cart",
        "Read an owned POS cart and its authoritative quote",
    ),
    ("pos.replace", "Revision-checked native POS cart edit"),
    (
        "pos.select",
        "Select customer/contact and a configured payment through the shared checkout",
    ),
    (
        "pos.accept",
        "Record explicit current legal acknowledgements through the shared legal owner",
    ),
    (
        "pos.complete",
        "Idempotent native POS checkout, cash tender in exact currency minor units",
    ),
    (
        "pos.held",
        "List this cashier's held carts in this sales channel",
    ),
    (
        "pos.sales",
        "Read recent channel orders and cash tender records",
    ),
];
pub fn schema(name: &str) -> Option<Value> {
    let op = name.strip_prefix("pos.")?;
    if !CAPABILITIES.iter().any(|(n, _)| *n == name) {
        return None;
    }
    if op == "channels" {
        return Some(
            json!({"type":"object","properties":{},"required":[],"additionalProperties":false}),
        );
    }
    let mut props = serde_json::Map::new();
    props.insert("channelId".into(), json!({"type":"string","maxLength":100}));
    let mut required = vec!["channelId"];
    if [
        "customer",
        "discount",
        "cart",
        "replace",
        "select",
        "accept",
        "complete",
        "currency",
        "coupons",
        "payment",
        "reconcile",
    ]
    .contains(&op)
    {
        props.insert("cartToken".into(), json!({"type":"string","maxLength":100}));
        required.push("cartToken");
    }
    match op {
        "customers" => {
            props.insert("query".into(), json!({"type":"string","maxLength":200}));
            props.insert("barcode".into(), json!({"type":"string","maxLength":100}));
        }
        "customer" => {
            props.insert("revision".into(), json!({"type":"integer"}));
            props.insert(
                "customerId".into(),
                json!({"type":["string","null"],"maxLength":100}),
            );
            required.extend(["revision", "customerId"]);
        }
        "discount" => {
            props.insert("revision".into(), json!({"type":"integer"}));
            props.insert("discount".into(), json!({"anyOf":[{"type":"null"},{"type":"object","properties":{"kind":{"enum":["percent","fixed"]},"value":{"type":"integer","minimum":0},"currency":{"type":"string","maxLength":3},"reason":{"type":"string","minLength":1,"maxLength":240}},"required":["kind","value","currency","reason"],"additionalProperties":false}]}));
            required.extend(["revision", "discount"]);
        }
        "receipt" => {
            props.insert("orderId".into(), json!({"type":"string"}));
            props.insert("receiptId".into(), json!({"type":"string"}));
            required.extend(["orderId", "receiptId"]);
        }
        "detail" => {
            props.insert("cartToken".into(), json!({"type":"string","maxLength":100}));
            props.insert("productId".into(), json!({"type":"string"}));
            props.insert("after".into(), json!({"type":"string"}));
            required.push("productId");
        }
        "catalog" => {
            props.insert("cartToken".into(), json!({"type":"string","maxLength":100}));
            props.insert("query".into(), json!({"type":"string","maxLength":200}));
            props.insert("after".into(), json!({"type":"string","maxLength":200}));
            props.insert("barcode".into(), json!({"type":"string","maxLength":100}));
        }
        "currency" | "coupons" => {
            props.insert("revision".into(), json!({"type":"integer"}));
            required.push("revision");
            if op == "currency" {
                props.insert("currency".into(), json!({"type":"string","maxLength":3}));
                required.push("currency");
            } else {
                props.insert(
                    "codes".into(),
                    json!({"type":"array","maxItems":5,"items":{"type":"string","maxLength":64}}),
                );
                required.push("codes");
            }
        }
        "replace" => {
            props.insert("revision".into(), json!({"type":"integer"}));
            props.insert("items".into(), json!({"type":"array","maxItems":100,"items":{"type":"object","properties":{"id":{"type":"string"},"quantity":{"type":"integer","minimum":1}},"required":["id","quantity"],"additionalProperties":false}}));
            required.extend(["revision", "items"]);
        }
        "select" => {
            props.insert("revision".into(), json!({"type":"integer"}));
            props.insert("checkout".into(), json!({"type":"object"}));
            required.extend(["revision", "checkout"]);
        }
        "accept" => {
            for k in ["terms", "digitalImmediate"] {
                props.insert(k.into(), json!({"type":"boolean"}));
            }
            props.insert("policyVersion".into(), json!({"type":"string"}));
            required.extend(["terms", "digitalImmediate", "policyVersion"]);
        }
        "reconcile" => {
            props.insert(
                "requestKey".into(),
                json!({"type":"string","minLength":8,"maxLength":128}),
            );
            required.push("requestKey");
        }
        "complete" => {
            props.insert("revision".into(), json!({"type":"integer"}));
            props.insert(
                "requestKey".into(),
                json!({"type":"string","minLength":8,"maxLength":128}),
            );
            props.insert(
                "totalMinor".into(),
                json!({"type":"string","pattern":"^[0-9]{1,13}$"}),
            );
            props.insert(
                "tenderMinor".into(),
                json!({"type":"string","pattern":"^[0-9]{1,13}$"}),
            );
            required.extend(["revision", "requestKey", "totalMinor"]);
        }
        _ => {}
    }
    Some(
        json!({"type":"object","properties":props,"required":required,"additionalProperties":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::{CAPABILITIES, schema};
    use serde_json::{Value, json};

    /// The schema builder before it used a typed property map; the shapes must stay identical.
    fn old_schema(name: &str) -> Option<Value> {
        let op = name.strip_prefix("pos.")?;
        if !CAPABILITIES.iter().any(|(n, _)| *n == name) {
            return None;
        }
        if op == "channels" {
            return Some(
                json!({"type":"object","properties":{},"required":[],"additionalProperties":false}),
            );
        }
        let mut props = json!({"channelId":{"type":"string","maxLength":100}});
        let mut required = vec!["channelId"];
        if [
            "customer",
            "discount",
            "cart",
            "replace",
            "select",
            "accept",
            "complete",
            "currency",
            "coupons",
            "payment",
            "reconcile",
        ]
        .contains(&op)
        {
            props["cartToken"] = json!({"type":"string","maxLength":100});
            required.push("cartToken");
        }
        match op {
            "customers" => {
                props["query"] = json!({"type":"string","maxLength":200});
                props["barcode"] = json!({"type":"string","maxLength":100});
            }
            "customer" => {
                props["revision"] = json!({"type":"integer"});
                props["customerId"] = json!({"type":["string","null"],"maxLength":100});
                required.extend(["revision", "customerId"]);
            }
            "discount" => {
                props["revision"] = json!({"type":"integer"});
                props["discount"] = json!({"anyOf":[{"type":"null"},{"type":"object","properties":{"kind":{"enum":["percent","fixed"]},"value":{"type":"integer","minimum":0},"currency":{"type":"string","maxLength":3},"reason":{"type":"string","minLength":1,"maxLength":240}},"required":["kind","value","currency","reason"],"additionalProperties":false}]});
                required.extend(["revision", "discount"]);
            }
            "receipt" => {
                props["orderId"] = json!({"type":"string"});
                props["receiptId"] = json!({"type":"string"});
                required.extend(["orderId", "receiptId"]);
            }
            "detail" => {
                props["cartToken"] = json!({"type":"string","maxLength":100});
                props["productId"] = json!({"type":"string"});
                props["after"] = json!({"type":"string"});
                required.push("productId");
            }
            "catalog" => {
                props["cartToken"] = json!({"type":"string","maxLength":100});
                props["query"] = json!({"type":"string","maxLength":200});
                props["after"] = json!({"type":"string","maxLength":200});
                props["barcode"] = json!({"type":"string","maxLength":100});
            }
            "currency" | "coupons" => {
                props["revision"] = json!({"type":"integer"});
                required.push("revision");
                if op == "currency" {
                    props["currency"] = json!({"type":"string","maxLength":3});
                    required.push("currency");
                } else {
                    props["codes"] = json!({"type":"array","maxItems":5,"items":{"type":"string","maxLength":64}});
                    required.push("codes");
                }
            }
            "replace" => {
                props["revision"] = json!({"type":"integer"});
                props["items"] = json!({"type":"array","maxItems":100,"items":{"type":"object","properties":{"id":{"type":"string"},"quantity":{"type":"integer","minimum":1}},"required":["id","quantity"],"additionalProperties":false}});
                required.extend(["revision", "items"]);
            }
            "select" => {
                props["revision"] = json!({"type":"integer"});
                props["checkout"] = json!({"type":"object"});
                required.extend(["revision", "checkout"]);
            }
            "accept" => {
                for k in ["terms", "digitalImmediate"] {
                    props[k] = json!({"type":"boolean"});
                }
                props["policyVersion"] = json!({"type":"string"});
                required.extend(["terms", "digitalImmediate", "policyVersion"]);
            }
            "reconcile" => {
                props["requestKey"] = json!({"type":"string","minLength":8,"maxLength":128});
                required.push("requestKey");
            }
            "complete" => {
                props["revision"] = json!({"type":"integer"});
                props["requestKey"] = json!({"type":"string","minLength":8,"maxLength":128});
                props["totalMinor"] = json!({"type":"string","pattern":"^[0-9]{1,13}$"});
                props["tenderMinor"] = json!({"type":"string","pattern":"^[0-9]{1,13}$"});
                required.extend(["revision", "requestKey", "totalMinor"]);
            }
            _ => {}
        }
        Some(
            json!({"type":"object","properties":props,"required":required,"additionalProperties":false}),
        )
    }

    #[test]
    fn every_operation_has_a_closed_schema_bound_to_its_channel() {
        for (name, _) in CAPABILITIES {
            let s = schema(name).unwrap();
            assert_eq!(s["type"], "object", "{name}");
            assert_eq!(s["additionalProperties"], false, "{name}");
            let required = s["required"].as_array().unwrap();
            assert_eq!(
                required.iter().any(|key| key == "channelId"),
                *name != "pos.channels",
                "{name}"
            );
            for key in required {
                assert!(
                    s["properties"][key.as_str().unwrap()].is_object(),
                    "{name} {key}"
                );
            }
        }
        assert!(schema("pos.refund.everything").is_none());
        for (name, _) in CAPABILITIES {
            assert_eq!(schema(name), old_schema(name), "{name}");
        }
        assert!(schema("channels").is_none());
    }
}
