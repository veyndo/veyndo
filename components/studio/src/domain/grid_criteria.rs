//! One allowlisted list-filter schema for model plans and MCP; no model-supplied SQL or tenant identity.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_app_platform::domain::extension_filter::decode;
use veyndo_catalog::types::product_list::ProductCriteria;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};
use veyndo_orders::types::list_criteria::Criteria;

/// One filter field of a grid schema, serialized as the JSON Schema of that field.
/// Field order follows the documents the schema was built from: type, enum, maxLength, minLength.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Field {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    allowed: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_length: Option<u32>,
}

impl Field {
    fn of(kind: &'static str) -> Self {
        Field {
            kind,
            allowed: None,
            max_length: None,
            min_length: None,
        }
    }
    fn text(max_length: u32) -> Self {
        Field {
            max_length: Some(max_length),
            ..Field::of("string")
        }
    }
    fn choice(kind: &'static str, allowed: Value) -> Self {
        Field {
            allowed: Some(allowed),
            ..Field::of(kind)
        }
    }
}

/// The typed filter fields and sort keys of one grid entity, in schema order.
fn field_list(entity: &str) -> Result<Vec<(&'static str, Field)>> {
    let (texts, flags, sorts): (&[&'static str], &[&'static str], &[&str]) = match entity {
        "orders" => (
            &[
                "query",
                "customer",
                "state",
                "paymentState",
                "deliveryState",
                "channelId",
                "from",
                "to",
            ],
            &[],
            &["created", "number", "customer", "total", "state"],
        ),
        "customers" => (
            &["query", "group"],
            &["guest", "active"],
            &["email", "name", "created", "number"],
        ),
        "products" => (
            &["search", "categoryId", "parentId"],
            &["active", "lowStock"],
            &["id", "name", "number", "price", "stock"],
        ),
        _ => return Err(bad("Unknown grid entity")),
    };
    let mut list = vec![
        ("sort", Field::choice("string", json!(sorts))),
        ("direction", Field::choice("string", json!(["asc", "desc"]))),
    ];
    list.extend(texts.iter().map(|key| (*key, Field::text(200))));
    list.extend(flags.iter().map(|key| (*key, Field::of("boolean"))));
    list.push(("appFilter", Field::text(4096)));
    Ok(list)
}

/// The filter fields and sort keys of one grid entity: orders, customers or products.
pub fn fields(entity: &str) -> Result<Value> {
    Ok(object(field_list(entity)?))
}

/// The JSON Schema properties object of a field list.
fn object(list: Vec<(&'static str, Field)>) -> Value {
    Value::Object(
        list.into_iter()
            .map(|(key, field)| (key.to_owned(), json!(field)))
            .collect(),
    )
}

/// The strict model schema of one entity's filters: non-empty strings, no false stock filter.
pub fn schema(entity: &str) -> Result<Value> {
    let mut list = field_list(entity)?;
    for (key, field) in &mut list {
        if field.kind == "string" {
            field.min_length = Some(1);
        }
        if entity == "products" && *key == "lowStock" {
            field.allowed = Some(json!([true]));
        }
    }
    Ok(json!({"type":"object","properties":object(list),"additionalProperties":false}))
}
/// The JSON fields `validate` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateInput {
    #[serde(deserialize_with = "lenient")]
    low_stock: Option<bool>,
}

/// Admits caller or model criteria and returns them without null placeholders.
pub fn validate(entity: &str, v: &Value) -> Result<Value> {
    let input: ValidateInput = view(v);
    if entity == "products" && input.low_stock == Some(false) {
        return Err(bad("lowStock=false does not express a stock constraint"));
    }
    let fields = field_list(entity)?;
    let object = v.as_object().ok_or(bad("Invalid grid criteria"))?;
    let mut normalized = serde_json::Map::new();
    for (key, value) in object {
        let field = fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, field)| field)
            .ok_or(bad("Unsupported grid filter"))?;
        // Strict provider schemas represent omitted optional fields as null.
        // Check the key first: an unknown identity field must never be erased.
        if value.is_null() {
            continue;
        }
        if key == "appFilter" {
            decode(value.as_str().ok_or(bad("Invalid app filter"))?)?;
        } else if field.kind == "boolean" {
            if !value.is_boolean() {
                return Err(bad("Invalid boolean grid filter"));
            }
        } else {
            let text = value.as_str().ok_or(bad("Invalid text grid filter"))?;
            if text.trim().is_empty()
                || text.len() > 200
                || text.chars().any(char::is_control)
                || field
                    .allowed
                    .as_ref()
                    .and_then(Value::as_array)
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
    /// The schemas as the indexed construction built them.
    fn indexed(entity: &str) -> (Value, Value) {
        let string = json!({"type":"string","maxLength":200});
        let mut p =
            json!({"sort":{"type":"string"},"direction":{"type":"string","enum":["asc","desc"]}});
        let (texts, flags, sorts): (&[&str], &[&str], &[&str]) = match entity {
            "orders" => (
                &[
                    "query",
                    "customer",
                    "state",
                    "paymentState",
                    "deliveryState",
                    "channelId",
                    "from",
                    "to",
                ],
                &[],
                &["created", "number", "customer", "total", "state"],
            ),
            "customers" => (
                &["query", "group"],
                &["guest", "active"],
                &["email", "name", "created", "number"],
            ),
            _ => (
                &["search", "categoryId", "parentId"],
                &["active", "lowStock"],
                &["id", "name", "number", "price", "stock"],
            ),
        };
        for key in texts {
            p[*key] = string.clone();
        }
        for key in flags {
            p[*key] = json!({"type":"boolean"});
        }
        p["appFilter"] = json!({"type":"string","maxLength":4096});
        p["sort"]["enum"] = json!(sorts);
        let mut fields = p.clone();
        for field in fields.as_object_mut().unwrap().values_mut() {
            if field["type"] == "string" {
                field["minLength"] = json!(1);
            }
        }
        if entity == "products" {
            fields["lowStock"]["enum"] = json!([true]);
        }
        (
            p,
            json!({"type":"object","properties":fields,"additionalProperties":false}),
        )
    }
    #[test]
    fn typed_schemas_keep_the_indexed_shape() {
        for entity in ["orders", "customers", "products"] {
            let (fields_before, schema_before) = indexed(entity);
            assert_eq!(
                fields(entity).unwrap().to_string(),
                fields_before.to_string()
            );
            assert_eq!(
                schema(entity).unwrap().to_string(),
                schema_before.to_string()
            );
        }
    }
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
            json!({"sql":"DROP SCHEMA public CASCADE"}),
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
