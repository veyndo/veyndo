//! One catalogue-intake contract is shared by HTTP, MCP and explicit UI review; it cannot publish or overwrite products.
use serde_json::{Value, json};
pub const CAPABILITIES: &[(&str, &str)] = &[
    (
        "merchant.catalog.context",
        "Read current catalogue intake language, currency, taxes and sandbox status",
    ),
    (
        "merchant.catalog.prepare",
        "Prepare CSV, public URL or described demo products without saving",
    ),
    (
        "merchant.catalog.save",
        "Explicitly save one reviewed new product in the current private sandbox",
    ),
];
pub fn schema(name: &str) -> Option<Value> {
    let (props, required) = match name {
        "merchant.catalog.context" => (json!({}), vec![]),
        "merchant.catalog.prepare" => (
            json!({"mode":{"type":"string","enum":["csv","url","demo","shopify_csv","shop_mcp"]},"source":{"type":"string","minLength":1,"maxLength":200000},"shopDomain":{"type":"string","maxLength":253},"count":{"type":"integer","minimum":1,"maximum":10},"inference":{"type":["object","null"],"properties":{"provider":{"type":"string","enum":["platform","ollama","openai","anthropic"]},"model":{"type":["string","null"]}},"additionalProperties":false}}),
            vec!["mode", "source", "count"],
        ),
        "merchant.catalog.save" => (
            json!({"id":{"type":"string"},"approve":{"type":"boolean"},"settingsRevision":{"type":"integer","minimum":1},"mainLocale":{"type":"string"},"currency":{"type":"string"},"taxRate":{"type":"number","minimum":0,"maximum":50},"demo":{"type":"boolean"},"draft":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","maxLength":4000},"price":{"type":"number","minimum":0,"maximum":1000000},"productNumber":{"type":"string","maxLength":100},"imageUrl":{"type":"string","maxLength":2000},"source":{"type":"object","additionalProperties":false,"properties":{"provider":{"type":"string","enum":["shopify_csv","shop_mcp"]},"externalId":{"type":"string","maxLength":500},"url":{"type":"string","maxLength":2000}},"required":["provider","externalId"]}},"required":["name","description","price"]}}),
            vec![
                "id",
                "approve",
                "settingsRevision",
                "mainLocale",
                "currency",
                "taxRate",
                "demo",
                "draft",
            ],
        ),
        _ => return None,
    };
    Some(
        json!({"type":"object","properties":props,"required":required,"additionalProperties":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::{CAPABILITIES, schema};

    #[test]
    fn every_intake_tool_has_a_closed_schema_whose_required_fields_exist() {
        for (name, _) in CAPABILITIES {
            let s = schema(name).unwrap();
            assert_eq!(s["type"], "object", "{name}");
            assert_eq!(s["additionalProperties"], false, "{name}");
            for key in s["required"].as_array().unwrap() {
                assert!(
                    s["properties"][key.as_str().unwrap()].is_object(),
                    "{name} {key}"
                );
            }
        }
        let save = schema("merchant.catalog.save").unwrap();
        assert_eq!(save["properties"]["draft"]["additionalProperties"], false);
        assert_eq!(
            schema("merchant.catalog.prepare").unwrap()["properties"]["count"]["maximum"],
            10
        );
        assert!(schema("merchant.catalog.publish").is_none());
    }
}
