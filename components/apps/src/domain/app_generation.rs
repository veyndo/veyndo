//! The manifest schema a model answers in, specialised to the shop's languages, and the access a regeneration keeps.
//! Draft members the host reads are typed views; the draft and its JSON Schema stay open JSON otherwise and are
//! typed only by native `Manifest` validation.
use crate::domain::draft_document::{Named, copy, member, present, schema_at, write};
use crate::domain::native_lifecycle::generation_context;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, lenient_list, view};
use veyndo_core::{Result, bad};
use veyndo_identity::SCOPES;

/// The draft members `default_actions` reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ActionDraft {
    #[serde(deserialize_with = "present")]
    actions: Option<Value>,
    entities: Value,
    payment_provider: Value,
}

/// An entity as default actions read it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ActionEntity {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    fields: Value,
    public_read: Value,
}

/// A field as default actions read it: only its native reference.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReferenceField {
    #[serde(deserialize_with = "lenient")]
    core_reference: Option<String>,
}

#[derive(Serialize)]
struct Actions {
    actions: Value,
}

/// Adds the list and save actions every entity gets when a draft declares none; payment providers also get the supplied actions.
pub fn default_actions(m: &mut Value, payment: &[Value]) -> Result<()> {
    let draft: ActionDraft = view(m);
    if draft.actions.is_some() {
        return Ok(());
    }
    let mut list = vec![];
    for row in draft.entities.as_array().ok_or(bad("Entities required"))? {
        let e: ActionEntity = view(row);
        let name = e.name.as_deref().ok_or(bad("Entity name required"))?;
        let core = e.fields.as_array().and_then(|fields| {
            fields
                .iter()
                .find_map(|f| view::<ReferenceField>(f).core_reference)
        });
        let scope = match core.as_deref() {
            Some("customer") => "customers",
            Some("order") => "orders",
            _ => "catalog",
        };
        let public = e.public_read == true;
        list.push(json!({"name":format!("list_{}",name.chars().take(27).collect::<String>()),"description":format!("List {name}"),"handler":"list","entity":name,"readOnly":true,"public":public,"mcp":false,"permission":if public {None}else{Some(format!("{scope}.read"))},"inputSchema":{"type":"object","properties":{"limit":{"type":"integer"},"after":{"type":"string"},"filter":{"type":"object"}},"additionalProperties":false}}));
        list.push(json!({"name":format!("save_{}",name.chars().take(27).collect::<String>()),"description":format!("Save {name}"),"handler":"save","entity":name,"public":false,"mcp":false,"permission":format!("{scope}.write"),"flowAllowed":true,"inputSchema":{"type":"object","properties":{"id":{"type":"string"},"revision":{"type":"integer"},"fields":{"type":"object"}},"required":["id","fields"],"additionalProperties":false}}));
    }
    if draft.payment_provider.is_object() {
        list.extend(payment.iter().cloned());
    }
    write(
        m,
        Actions {
            actions: json!(list),
        },
    )
}
/// The JSON fields `preserve_access` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PreserveAccessInput {
    id: Value,
    #[serde(deserialize_with = "lenient_list")]
    actions: Vec<PriorAction>,
}

/// The access members of an accepted action that a regeneration keeps.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PriorAction {
    name: Value,
    #[serde(deserialize_with = "present")]
    mcp: Option<Value>,
    #[serde(deserialize_with = "present")]
    permission: Option<Value>,
    #[serde(deserialize_with = "present")]
    required_permissions: Option<Value>,
    #[serde(deserialize_with = "present")]
    public: Option<Value>,
    #[serde(deserialize_with = "present")]
    flow_allowed: Option<Value>,
}

/// The draft member `preserve_access` compares before patching.
#[derive(Deserialize, Default)]
#[serde(default)]
struct DraftIdentity {
    id: Value,
}

/// Restricted model generation does not silently reopen existing actions to agents or weaken team scopes.
pub fn preserve_access(old: &Value, next: &mut Value) {
    let input: PreserveAccessInput = view(old);
    if input.id != view::<DraftIdentity>(next).id {
        return;
    }
    // A draft that is not an object has no actions to patch; indexing it would have panicked.
    let Ok(actions) = member(next, "actions") else {
        return;
    };
    for action in actions.as_array_mut().into_iter().flatten() {
        let name = view::<Named>(action).name;
        if let Some(prior) = input.actions.iter().find(|a| a.name == name) {
            let mcp = prior.mcp.clone().unwrap_or(Value::Bool(true));
            let retained = [
                ("mcp", Some(&mcp)),
                ("permission", prior.permission.as_ref()),
                ("requiredPermissions", prior.required_permissions.as_ref()),
                ("public", prior.public.as_ref()),
                ("flowAllowed", prior.flow_allowed.as_ref()),
            ];
            for (key, value) in retained {
                if value.is_some() && copy(action, key, value).is_err() {
                    return;
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

/// The localized-text keywords of a JSON Schema node, replaced by the configured languages.
#[derive(Serialize)]
struct LocalizedText<'a> {
    properties: Value,
    required: &'a [String],
}

/// Shared schema specializations use actual configured languages, including regional locales.
pub fn schema_for(locales: &[String]) -> Value {
    fn localized(v: &Value) -> bool {
        v.get("type").is_some_and(|t| t == "object")
            && v.get("properties")
                .is_some_and(|p| p.get("en").is_some() && p.get("de").is_some())
    }
    fn walk(v: &mut Value, locales: &[String]) {
        if localized(v) {
            let props = locales
                .iter()
                .map(|l| (l.clone(), json!({"type":"string"})))
                .collect::<serde_json::Map<_, _>>();
            write(
                v,
                LocalizedText {
                    properties: Value::Object(props),
                    required: locales,
                },
            )
            .expect("a localized schema node is an object");
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
    let mut specialise = || -> Result<()> {
        let lifecycle = schema_at(
            &mut v,
            &["properties", "manifest", "properties", "lifecycles"],
        )?;
        *member(lifecycle, "description")? = json!(generation_context().to_string());
        *schema_at(
            lifecycle,
            &[
                "items",
                "properties",
                "transitions",
                "items",
                "properties",
                "requiredPermissions",
                "items",
                "enum",
            ],
        )? = json!(SCOPES);
        Ok(())
    };
    specialise().expect("bundled app studio schema declares native lifecycles");
    v
}

#[cfg(test)]
/// The untyped implementation this module replaced, kept as the oracle for the JSON equivalence test.
mod legacy {
    use super::schema;
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
#[cfg(test)]
mod equivalence {
    use serde_json::{Value, json};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn drafts() -> Vec<Value> {
        vec![
            json!({"id":"a","entities":[{"name":"cases","publicRead":true,"fields":[{"name":"o","coreReference":"order"}]},
                {"name":"people","fields":[{"coreReference":7},{"coreReference":"customer"}]},{"name":"plain","fields":"x"}],
                "paymentProvider":{"api":"1"}}),
            json!({"id":"a","entities":[{"name":"x"}],"paymentProvider":null}),
            json!({"id":"a","entities":[{"fields":[]}]}),
            json!({"id":"a","entities":"none"}),
            json!({"id":"a","actions":null,"entities":[]}),
            json!({"id":"a"}),
            Value::Null,
            json!(["not", "an", "object"]),
        ]
    }

    #[test]
    fn typed_default_actions_build_the_same_actions() {
        let payment = [json!({"name":"pay","handler":"service"})];
        for draft in drafts() {
            let mut typed = draft.clone();
            let mut untyped = draft.clone();
            let outcome = super::default_actions(&mut typed, &payment);
            match catch_unwind(AssertUnwindSafe(|| {
                super::legacy::default_actions(&mut untyped, &payment)
            })) {
                Ok(expected) => {
                    assert_eq!(outcome.is_ok(), expected.is_ok(), "{draft}");
                    if outcome.is_ok() {
                        assert_eq!(typed, untyped, "{draft}");
                    }
                }
                Err(_) => assert!(outcome.is_err(), "{draft}"),
            }
        }
    }

    #[test]
    fn typed_preserve_access_keeps_the_same_members() {
        let old = json!({"id":"a","actions":[{"name":"list","mcp":false,"permission":"orders.read","public":null},
            {"name":"save","requiredPermissions":["x"],"flowAllowed":true},"bare",{"name":"open"}]});
        let nexts = [
            json!({"id":"a","actions":[{"name":"list","mcp":true,"permission":null,"public":true},{"name":"save"},{"name":"open","mcp":false},{"name":"new"}]}),
            json!({"id":"a"}),
            json!({"id":"b","actions":[{"name":"list"}]}),
            json!({"id":"a","actions":"none"}),
            json!({"id":null}),
            Value::Null,
        ];
        for old in [old.clone(), Value::Null, json!({"actions":[{"mcp":false}]})] {
            for next in &nexts {
                let mut typed = next.clone();
                let mut untyped = next.clone();
                super::preserve_access(&old, &mut typed);
                super::legacy::preserve_access(&old, &mut untyped);
                assert_eq!(typed, untyped, "{old} {next}");
            }
        }
    }

    #[test]
    fn typed_schema_specialisation_builds_the_same_schema() {
        for locales in [
            vec!["en".to_string()],
            vec![
                "de-CH".into(),
                "en".into(),
                "es".into(),
                "fr".into(),
                "it".into(),
            ],
        ] {
            assert_eq!(
                super::schema_for(&locales),
                super::legacy::schema_for(&locales)
            );
        }
    }
}
