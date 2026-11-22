//! One allowlisted list-filter schema for model plans and MCP; no model-supplied SQL or tenant identity.
use serde_json::{Value, json};
use veyndo_app_platform::domain::extension_filter::decode;
use veyndo_catalog::types::product_list::ProductCriteria;
use veyndo_core::{Result, bad};
use veyndo_orders::types::list_criteria::Criteria;

/// The filter fields and sort keys of one grid entity: orders, customers or products.
pub fn fields(entity: &str) -> Result<Value> {
    let string = json!({"type":"string","maxLength":200});
    let mut p =
        json!({"sort":{"type":"string"},"direction":{"type":"string","enum":["asc","desc"]}});
    let sorts: &[&str] = match entity {
        "orders" => {
            for key in [
                "query",
                "customer",
                "state",
                "paymentState",
                "deliveryState",
                "channelId",
                "from",
                "to",
            ] {
                p[key] = string.clone();
            }
            &["created", "number", "customer", "total", "state"]
        }
        "customers" => {
            for key in ["query", "group"] {
                p[key] = string.clone();
            }
            for key in ["guest", "active"] {
                p[key] = json!({"type":"boolean"});
            }
            &["email", "name", "created", "number"]
        }
        "products" => {
            for key in ["search", "categoryId", "parentId"] {
                p[key] = string.clone();
            }
            for key in ["active", "lowStock"] {
                p[key] = json!({"type":"boolean"});
            }
            &["id", "name", "number", "price", "stock"]
        }
        _ => return Err(bad("Unknown grid entity")),
    };
    p["appFilter"] = json!({"type":"string","maxLength":4096});
    p["sort"]["enum"] = json!(sorts);
    Ok(p)
}
/// The strict model schema of one entity's filters: non-empty strings, no false stock filter.
pub fn schema(entity: &str) -> Result<Value> {
    let mut fields = fields(entity)?;
    for field in fields.as_object_mut().unwrap().values_mut() {
        if field["type"] == "string" {
            field["minLength"] = json!(1);
        }
    }
    if entity == "products" {
        fields["lowStock"]["enum"] = json!([true]);
    }
    Ok(json!({"type":"object","properties":fields,"additionalProperties":false}))
}
/// Admits caller or model criteria and returns them without null placeholders.
pub fn validate(entity: &str, v: &Value) -> Result<Value> {
    if entity == "products" && v["lowStock"] == false {
        return Err(bad("lowStock=false does not express a stock constraint"));
    }
    let fields = fields(entity)?;
    let object = v.as_object().ok_or(bad("Invalid grid criteria"))?;
    let mut normalized = serde_json::Map::new();
    for (key, value) in object {
        let field = fields.get(key).ok_or(bad("Unsupported grid filter"))?;
        // Strict provider schemas represent omitted optional fields as null.
        // Check the key first: an unknown identity field must never be erased.
        if value.is_null() {
            continue;
        }
        if key == "appFilter" {
            decode(value.as_str().ok_or(bad("Invalid app filter"))?)?;
        } else if field["type"] == "boolean" {
            if !value.is_boolean() {
                return Err(bad("Invalid boolean grid filter"));
            }
        } else {
            let text = value.as_str().ok_or(bad("Invalid text grid filter"))?;
            if text.trim().is_empty()
                || text.len() > 200
                || text.chars().any(char::is_control)
                || field["enum"]
                    .as_array()
                    .is_some_and(|items| !items.contains(value))
            {
                return Err(bad("Invalid grid filter value"));
            }
        }
        normalized.insert(key.clone(), value.clone());
    }
    let normalized = Value::Object(normalized);
    if entity == "products" {
        let c: ProductCriteria = serde_json::from_value(normalized.clone())
            .map_err(|_| bad("Invalid product filters"))?;
        c.validate()?;
        decode(&c.app_filter)?;
    } else {
        let c: Criteria =
            serde_json::from_value(normalized.clone()).map_err(|_| bad("Invalid list filters"))?;
        c.validate()?;
        decode(&c.app_filter)?;
        c.sort_for(entity)?;
    }
    Ok(normalized)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schemas_are_closed_per_entity() {
        assert!(fields("refunds").is_err());
        assert_eq!(fields("orders").unwrap()["sort"]["enum"][0], "created");
        let products = schema("products").unwrap();
        assert_eq!(products["additionalProperties"], false);
        assert_eq!(products["properties"]["lowStock"]["enum"], json!([true]));
        assert_eq!(products["properties"]["search"]["minLength"], 1);
    }
    #[test]
    fn plans_cannot_select_identity_sql_or_mutations() {
        for v in [
            json!({"tenant":"victim"}),
            json!({"sql":"DELETE FROM orders"}),
            json!({"after":"foreign"}),
            json!({"limit":999}),
            json!({"sort":"id; DROP TABLE orders"}),
        ] {
            assert!(validate("orders", &v).is_err());
        }
        assert!(
            validate(
                "orders",
                &json!({"customer":"Alex", "sort":"created", "direction":"desc"})
            )
            .is_ok()
        );
        assert!(validate("customers", &json!({"guest":true})).is_ok());
        assert!(validate("products", &json!({"active":true,"lowStock":true})).is_ok());
        assert!(validate("products", &json!({"lowStock":false})).is_err());
        assert_eq!(
            validate(
                "orders",
                &json!({"customer":"Alex", "state":null, "sort":null})
            )
            .unwrap(),
            json!({"customer":"Alex"})
        );
        assert!(validate("orders", &json!({"tenant":null})).is_err());
        assert!(validate("orders", &json!({"customer":""})).is_err());
        assert!(validate("orders", &json!({"query":"   "})).is_err());
        assert!(validate("customers", &json!({"state":"placed"})).is_err());
        assert!(validate("orders", &json!({"from":"not-a-date"})).is_err());
    }
}
