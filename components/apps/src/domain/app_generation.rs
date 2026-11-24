//! The manifest schema a model answers in, specialised to the shop's languages, and the access a regeneration keeps.
use crate::domain::native_lifecycle::generation_context;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::{Result, bad};
use veyndo_identity::SCOPES;
/// Adds the list and save actions every entity gets when a draft declares none; payment providers also get the supplied actions.
pub fn default_actions(m: &mut Value, payment: &[Value]) -> Result<()> {
    if m.get("actions").is_some() {
        return Ok(());
    }
    let mut list = vec![];
    for e in m["entities"].as_array().ok_or(bad("Entities required"))? {
        let name = e["name"].as_str().ok_or(bad("Entity name required"))?;
        let core = e["fields"]
            .as_array()
            .and_then(|fields| fields.iter().find_map(|f| f["coreReference"].as_str()));
        let scope = match core {
            Some("customer") => "customers",
            Some("order") => "orders",
            _ => "catalog",
        };
        let public = e["publicRead"] == true;
        list.push(json!({"name":format!("list_{}",name.chars().take(27).collect::<String>()),"description":format!("List {name}"),"handler":"list","entity":name,"readOnly":true,"public":public,"mcp":false,"permission":if public {None}else{Some(format!("{scope}.read"))},"inputSchema":{"type":"object","properties":{"limit":{"type":"integer"},"after":{"type":"string"},"filter":{"type":"object"}},"additionalProperties":false}}));
        list.push(json!({"name":format!("save_{}",name.chars().take(27).collect::<String>()),"description":format!("Save {name}"),"handler":"save","entity":name,"public":false,"mcp":false,"permission":format!("{scope}.write"),"flowAllowed":true,"inputSchema":{"type":"object","properties":{"id":{"type":"string"},"revision":{"type":"integer"},"fields":{"type":"object"}},"required":["id","fields"],"additionalProperties":false}}));
    }
    if m["paymentProvider"].is_object() {
        list.extend(payment.iter().cloned());
    }
    m["actions"] = json!(list);
    Ok(())
}
/// The JSON fields `preserve_access` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PreserveAccessInput {
    id: Value,
    actions: Value,
}

/// Restricted model generation does not silently reopen existing actions to agents or weaken team scopes.
pub fn preserve_access(old: &Value, next: &mut Value) {
    let input: PreserveAccessInput = view(old);
    if input.id != next["id"] {
        return;
    }
    if let Some(actions) = next["actions"].as_array_mut() {
        for action in actions {
            if let Some(prior) = input
                .actions
                .as_array()
                .and_then(|rows| rows.iter().find(|a| a["name"] == action["name"]))
            {
                action["mcp"] = prior.get("mcp").cloned().unwrap_or(Value::Bool(true));
                for key in [
                    "mcp",
                    "permission",
                    "requiredPermissions",
                    "public",
                    "flowAllowed",
                ] {
                    if let Some(value) = prior.get(key) {
                        action[key] = value.clone();
                    }
                }
            }
        }
    }
}
/// The bundled manifest schema a model answers in.
pub fn schema() -> Value {
    serde_json::from_str(include_str!("../../../../fixtures/app-studio-schema.json"))
        .expect("bundled app studio schema")
}

/// Shared schema specializations use actual configured languages, including regional locales.
pub fn schema_for(locales: &[String]) -> Value {
    fn walk(v: &mut Value, locales: &[String]) {
        if v["type"] == "object"
            && v["properties"].get("en").is_some()
            && v["properties"].get("de").is_some()
        {
            let props = locales
                .iter()
                .map(|l| (l.clone(), json!({"type":"string"})))
                .collect::<serde_json::Map<_, _>>();
            v["properties"] = Value::Object(props);
            v["required"] = json!(locales);
        } else {
            match v {
                Value::Object(o) => {
                    for value in o.values_mut() {
                        walk(value, locales);
                    }
                }
                Value::Array(a) => {
                    for value in a {
                        walk(value, locales);
                    }
                }
                _ => {}
            }
        }
    }
    let mut v = schema();
    walk(&mut v, locales);
    let lifecycle = &mut v["properties"]["manifest"]["properties"]["lifecycles"];
    lifecycle["description"] = json!(generation_context().to_string());
    lifecycle["items"]["properties"]["transitions"]["items"]["properties"]["requiredPermissions"]
        ["items"]["enum"] = json!(SCOPES);
    v
}

#[cfg(test)]
mod tests {
    use super::{default_actions, preserve_access, schema_for};
    use crate::domain::native_lifecycle;
    use serde_json::{Value, json};
    use veyndo_app_model::manifest::Manifest;
    use veyndo_identity::SCOPES;
    #[test]
    fn drafts_without_actions_get_scoped_list_and_save_actions() {
        let mut m = json!({"entities":[
            {"name":"notes","publicRead":true,"fields":[]},
            {"name":"visits","fields":[{"name":"who","coreReference":"customer"}]}
        ],"paymentProvider":{}});
        default_actions(&mut m, &[json!({"name":"provider_command"})]).unwrap();
        let actions = m["actions"].as_array().unwrap();
        assert_eq!(actions.len(), 5);
        assert_eq!(actions[0]["name"], "list_notes");
        assert_eq!(actions[0]["public"], true);
        assert!(actions[0]["permission"].is_null());
        assert_eq!(actions[1]["permission"], "catalog.write");
        assert_eq!(actions[2]["permission"], "customers.read");
        assert_eq!(actions[3]["permission"], "customers.write");
        assert_eq!(actions[4]["name"], "provider_command");
        let mut declared = json!({"actions":[],"entities":[{"name":"notes"}]});
        default_actions(&mut declared, &[]).unwrap();
        assert_eq!(declared["actions"], json!([]));
        assert!(default_actions(&mut json!({}), &[]).is_err());
        let mut plain = json!({"entities":[{"name":"notes"}]});
        default_actions(&mut plain, &[json!({"name":"provider_command"})]).unwrap();
        assert_eq!(plain["actions"].as_array().unwrap().len(), 2);
    }
    #[test]
    fn generated_handler_schema_requires_native_operations_and_record_inputs() {
        let schema = schema_for(&["it-IT".into()]);
        let handlers = &schema["properties"]["manifest"]["properties"]["views"]["items"]["properties"]
            ["blocks"]["items"]["properties"]["handlers"];
        let branches = handlers["properties"]["click"]["items"]["anyOf"]
            .as_array()
            .unwrap();
        assert_eq!(branches.len(), 5);
        for branch in branches {
            assert!(
                branch["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("op"))
            );
            assert_eq!(branch["additionalProperties"], false);
        }
        assert_eq!(
            branches[2]["properties"]["input"]["properties"]["kind"]["enum"],
            json!(["record"])
        );
        assert_eq!(
            branches[4]["properties"]["text"]["required"],
            json!(["it-IT"])
        );
        let base: Value = serde_json::from_str(include_str!(
            "../../../../extensions/apps/product-advisor/manifest.json"
        ))
        .unwrap();
        for instruction in [
            json!({"op":"validate","target":"editor"}),
            json!({"op":"refresh","target":"records"}),
            json!({"op":"call","action":"save_entries","input":{"kind":"record","block":"editor"}}),
            json!({"op":"navigate","view":"workspace"}),
            json!({"op":"msgBox","text":{"it-IT":"Salvato"}}),
        ] {
            let mut m = base.clone();
            m["views"][0]["blocks"][2]["handlers"]["click"] = json!([instruction]);
            assert!(serde_json::from_value::<Manifest>(m).is_ok());
        }
        let mut malformed = base;
        malformed["views"][0]["blocks"][2]["handlers"]["click"] =
            json!([{"action":"save_entries"}]);
        assert!(serde_json::from_value::<Manifest>(malformed).is_err());
    }
    #[test]
    fn generation_retains_access_of_existing_actions() {
        let old = json!({"id":"same","actions":[{"name":"save","mcp":false,"permission":"orders.write","requiredPermissions":["payments.manage","customers.pii"],"public":false,"flowAllowed":false}]});
        let mut next = json!({"id":"same","actions":[{"name":"save","mcp":true,"permission":"catalog.write","public":true,"flowAllowed":true}]});
        preserve_access(&old, &mut next);
        assert_eq!(next, old);
    }
    #[test]
    fn schema_uses_dynamic_regional_shop_languages() {
        let schema = schema_for(&["es-ES".into(), "it-IT".into(), "en-US".into()]);
        let labels = &schema["properties"]["manifest"]["properties"]["views"]["items"]["properties"]
            ["blocks"]["items"]["properties"]["title"];
        assert_eq!(labels["required"], json!(["es-ES", "it-IT", "en-US"]));
        assert!(labels["properties"].get("en").is_none());
        assert_eq!(labels["additionalProperties"], false);
        let description = &schema["properties"]["manifest"]["properties"]["presentation"]["properties"]
            ["description"];
        assert_eq!(description["required"], json!(["es-ES", "it-IT", "en-US"]));
        assert!(description["properties"].get("en").is_none());
    }
    #[test]
    fn workflow_generation_context_comes_from_native_validation_and_scope_owners() {
        let schema = schema_for(&["it-IT".into()]);
        let c = &schema["properties"]["manifest"]["properties"]["lifecycles"];
        let context: Value = serde_json::from_str(c["description"].as_str().unwrap()).unwrap();
        assert_eq!(context, native_lifecycle::generation_context());
        assert_eq!(context["quote"]["replaceableActions"], json!([]));
        assert_eq!(
            context["quote"]["allowedNativeTo"],
            json!(["requested", "rejected"])
        );
        assert_eq!(context["quote"]["guards"], json!(["quote.message"]));
        assert_eq!(
            c["items"]["properties"]["transitions"]["items"]["properties"]["requiredPermissions"]["items"]
                ["enum"],
            json!(SCOPES)
        );
        assert!(c.to_string().contains("it-IT"));
    }
}
