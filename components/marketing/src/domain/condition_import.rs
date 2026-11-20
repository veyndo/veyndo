//! Source-compatible condition import: converts original payloads without guessing identifiers or dropping configuration.
use serde_json::{Value, json};
use veyndo_core::{Result, bad};

/// Translates one source condition tree into the native shape; unknown types are kept as source conditions.
pub fn normalize(v: &Value, depth: usize) -> Result<Value> {
    if depth > 8 {
        return Err(bad("Maximum rule depth 8"));
    }
    let typ = v["type"].as_str().ok_or(bad("Condition type required"))?;
    if ["andContainer", "orContainer"].contains(&typ) {
        let children = v["children"]
            .as_array()
            .filter(|c| c.len() <= 20)
            .ok_or(bad("Container children required"))?;
        return Ok(
            json!({"type":typ,"children":children.iter().map(|c|normalize(c,depth+1)).collect::<Result<Vec<_>>>()?}),
        );
    }
    if typ == "notContainer" {
        let child = v
            .get("child")
            .or_else(|| {
                v["children"]
                    .as_array()
                    .filter(|c| c.len() == 1)
                    .and_then(|c| c.first())
            })
            .ok_or(bad("NOT requires one child"))?;
        return Ok(json!({"type":typ,"child":normalize(child,depth+1)?}));
    }
    let mut result = if v["value"].is_object() {
        v["value"].clone()
    } else {
        let mut copy = v.clone();
        for k in ["id", "parentId", "ruleId", "position", "children"] {
            copy.as_object_mut()
                .ok_or(bad("Condition object required"))?
                .remove(k);
        }
        copy
    };
    if veyndo_automation::rules::definition(typ).is_some() && typ != "alwaysValid" {
        let node = source_node(v, depth)?;
        return Ok(json!({"type":"shopwareCondition","name":typ,"config":node["config"]}));
    }
    let alias = match typ {
        "customerGroup" | "customerCustomerGroup" => Some("customerGroupIds"),
        "shippingCountry" | "customerBillingCountry" | "customerShippingCountry" => {
            Some("countryIds")
        }
        "shippingMethod" => Some("shippingMethodIds"),
        "paymentMethod" => Some("paymentMethodIds"),
        "salesChannel" => Some("salesChannelIds"),
        "lineItemId" | "cartLineItem" => Some("identifiers"),
        _ => None,
    };
    if let Some(key) = alias
        && let Some(value) = result.as_object_mut().and_then(|v| v.remove(key))
    {
        result["values"] = value;
    }
    result["type"] = json!(typ);
    Ok(result)
}

/// Convert original source payloads recursively without guessing identifiers or dropping configuration.
fn source_node(v: &Value, depth: usize) -> Result<Value> {
    if depth > 8 {
        return Err(bad("Maximum rule depth 8"));
    }
    let name = v["type"]
        .as_str()
        .ok_or(bad("Source condition type required"))?;
    let mut config = v
        .get("value")
        .filter(|v| v.is_object())
        .cloned()
        .unwrap_or_else(|| v.clone());
    let object = config
        .as_object_mut()
        .ok_or(bad("Source condition object required"))?;
    for key in ["type", "id", "ruleId", "parentId", "position"] {
        object.remove(key);
    }
    if let Some(children) = v
        .get("children")
        .or_else(|| config.get("children"))
        .cloned()
    {
        config["children"] = json!(
            children
                .as_array()
                .ok_or(bad("Children array required"))?
                .iter()
                .map(|c| source_node(c, depth + 1))
                .collect::<Result<Vec<_>>>()?
        );
    }
    for key in ["container", "filter"] {
        if let Some(child) = config.get(key).filter(|c| !c.is_null()).cloned() {
            config[key] = source_node(&child, depth + 1)?;
        }
    }
    veyndo_automation::rules::validate(name, &config, depth).map_err(bad)?;
    Ok(json!({"type":name,"config":config}))
}

#[cfg(test)]
mod tests {
    use super::normalize;
    use serde_json::json;
    #[test]
    fn original_condition_import_preserves_negative_sets_and_rejects_unsupported() {
        let c=normalize(&json!({"type":"customerGroup","value":{"customerGroupIds":["consumer"],"operator":"!="},"id":"ignored"}),0).unwrap();
        assert_eq!(
            c,
            json!({"type":"customerGroup","values":["consumer"],"operator":"!="})
        );
        assert!(normalize(&json!({"type":"notContainer","children":[]}), 0).is_err());
        assert!(normalize(&json!({"type":"notContainer","children":[{"type":"alwaysValid"},{"type":"alwaysValid"}]}),0).is_err());
    }
}
