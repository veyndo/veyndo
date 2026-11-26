//! Compile a bounded new-app brief into the existing native Manifest; no model-authored wiring or new executor.
//! The brief envelope, its entities and placements are typed as the host reads them; the entity and setting
//! declarations pass through as open JSON that only native `Manifest` validation types.
use crate::domain::draft_document::{Named, schema_at, schema_node};
use crate::domain::{app_generation, generation_bindings, generation_fields};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};

/// The draft members that decide whether guided creation may start.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Shell {
    runtime: Value,
    id: Value,
    #[serde(deserialize_with = "lenient")]
    version: Option<String>,
}

/// Whether a draft is still the empty declarative shell guided creation starts from.
pub fn empty(m: &Value) -> bool {
    view::<Shell>(m).runtime == "declarative"
        && [
            "entities",
            "views",
            "surfaces",
            "actions",
            "slots",
            "events",
            "lifecycles",
            "settings",
        ]
        .iter()
        .all(|k| {
            m.get(k)
                .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
        })
        && [
            "paymentProvider",
            "configuration",
            "capability",
            "distribution",
            "presentation",
        ]
        .iter()
        .all(|key| m.get(key).is_none_or(Value::is_null))
}
/// The bounded brief a model fills in for a new app, in the languages of the shop.
pub fn schema(locales: &[String]) -> Value {
    let base = app_generation::schema_for(locales);
    let summary = schema_node(&base, &["properties", "summary"]);
    let prop = |key: &str| schema_node(&base, &["properties", "manifest", "properties", key]);
    let name = prop("name");
    let mut entities = prop("entities");
    let mut constrain = || -> Result<()> {
        *schema_at(&mut entities, &["minItems"])? = json!(1);
        *schema_at(&mut entities, &["maxItems"])? = json!(3);
        let item = schema_at(&mut entities, &["items", "properties"])?;
        *schema_at(item, &["publicRead"])? = json!({"type":"boolean","enum":[false]});
        *schema_at(item, &["extends"])? = json!({"type":"null"});
        let fields = schema_at(item, &["fields"])?;
        *schema_at(fields, &["maxItems"])? = json!(16);
        let field = schema_at(fields, &["items"])?;
        *field = generation_fields::schema(field);
        Ok(())
    };
    constrain().expect("bundled app studio schema declares entities");
    let definition = json!({"type":"object","additionalProperties":false,
        "required":["name","entities","settings","placements"],"properties":{
        "name":name,"entities":entities,"settings":prop("settings"),
        "placements":{"type":"array","minItems":1,"maxItems":6,"items":{"type":"object","additionalProperties":false,
            "required":["location","entity","label","saveLabel"],"properties":{
            "location":{"type":"string","enum":["admin.navigation","admin.product.tab","admin.customer","admin.order","admin.b2b.company","admin.b2b.unit"]},
            "entity":{"type":"string"},"label":name,"saveLabel":name}}}}});
    json!({"type":"object","additionalProperties":false,"required":["summary","manifest"],"properties":{"summary":summary,"manifest":definition}})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Brief {
    name: HashMap<String, String>,
    summary: HashMap<String, String>,
    entities: Vec<Value>,
    settings: Vec<Value>,
    placements: Vec<Placement>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Placement {
    location: String,
    entity: String,
    label: HashMap<String, String>,
    save_label: HashMap<String, String>,
}
/// The two members of a brief answer; `apply` checks that nothing else is present.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Envelope {
    summary: Value,
    manifest: Value,
}
/// A brief entity as the host bounds and binds it; the declaration itself stays open JSON.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct BriefEntity {
    name: Value,
    public_read: Value,
    extends: Option<Value>,
    fields: Value,
}
/// A brief field as contextual placements match it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct BriefField {
    name: Value,
    core_reference: Value,
    indexed: Value,
    kind: Value,
}
/// A stored version of an app, as the version search reads it.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct VersionRow {
    pub app: Value,
    pub version: Value,
}
/// Compiles a brief into a complete native manifest; only an empty private draft accepts it.
pub fn apply(old: &Value, output: &Value, versions: &Value) -> Result<Value> {
    if !empty(old) {
        return Err(bad("Guided creation requires an empty private draft"));
    }
    if output
        .as_object()
        .is_none_or(|o| o.len() != 2 || !o.contains_key("summary") || !o.contains_key("manifest"))
    {
        return Err(bad("Invalid guided app brief envelope"));
    }
    let envelope: Envelope = view(output);
    let mut definition = envelope.manifest;
    definition
        .as_object_mut()
        .ok_or(bad("Invalid guided app definition"))?
        .insert("summary".into(), envelope.summary);
    let brief: Brief =
        serde_json::from_value(definition).map_err(|_| bad("Invalid guided app brief"))?;
    if brief.entities.is_empty()
        || brief.entities.len() > 3
        || brief.settings.len() > 16
        || brief.placements.is_empty()
        || brief.placements.len() > 6
        || brief.entities.iter().any(|e| {
            let e: BriefEntity = view(e);
            e.public_read != false
                || e.extends.is_some_and(|v| !v.is_null())
                || e.fields.as_array().is_none_or(|f| f.len() > 16)
        })
    {
        return Err(bad(
            "Guided apps need bounded private models and placements",
        ));
    }
    let shell: Shell = view(old);
    let mut version = semver::Version::parse(shell.version.as_deref().unwrap_or("0.1.0"))
        .map_err(|_| bad("Invalid draft version"))?;
    let stored: Vec<VersionRow> = crate::domain::draft_document::elements(versions);
    while stored
        .iter()
        .any(|v| v.app == shell.id && v.version == version.to_string())
    {
        version.patch = version
            .patch
            .checked_add(1)
            .ok_or(bad("App version limit exceeded"))?;
    }
    let mut views = Vec::new();
    let mut surfaces = Vec::new();
    for (i, p) in brief.placements.iter().enumerate() {
        let entity: BriefEntity = brief
            .entities
            .iter()
            .find(|e| view::<Named>(e).name == p.entity)
            .map(view)
            .ok_or(bad("Placement needs an existing data model"))?;
        let binding = match p.location.as_str() {
            "admin.navigation" => None,
            "admin.product.tab" => Some(("product", "productId")),
            "admin.customer" => Some(("customer", "customerId")),
            "admin.order" => Some(("order", "orderId")),
            "admin.b2b.company" => Some(("b2bCompany", "companyId")),
            "admin.b2b.unit" => Some(("b2bUnit", "unitId")),
            _ => {
                return Err(bad(
                    "Guided app placement must be a registered private admin location",
                ));
            }
        };
        let context = if let Some((core, key)) = binding {
            let field = entity
                .fields
                .as_array()
                .into_iter()
                .flatten()
                .map(view::<BriefField>)
                .find(|f| f.core_reference == core && f.indexed == true && f.kind == "string")
                .ok_or(bad(
                    "Contextual placement needs an indexed native reference field",
                ))?;
            json!({"field":field.name,"key":key})
        } else {
            Value::Null
        };
        let suffix = p.entity.chars().take(27).collect::<String>();
        let read = format!("list_{suffix}");
        let write = format!("save_{suffix}");
        let id = format!("view_{i}");
        views.push(json!({"id":id,"layout":"form","blocks":[
            {"id":"records","kind":"table","title":p.label,"entity":p.entity,"readAction":read,"contextBinding":context,"geometry":{"x":0,"y":0,"w":6,"h":10}},
            {"id":"editor","kind":"form","title":p.label,"entity":p.entity,"readAction":read,"writeAction":write,"contextBinding":context,"geometry":{"x":6,"y":0,"w":6,"h":10}},
            {"id":"save","kind":"button","title":p.save_label,"geometry":{"x":6,"y":10,"w":6,"h":2},"handlers":{"click":[{"op":"validate","target":"editor"},{"op":"call","action":write,"input":{"kind":"record","block":"editor"}},{"op":"refresh","target":"records"}]}}
        ]}));
        surfaces.push(json!({"id":id,"location":p.location,"label":p.label,"uiPath":format!("native/{id}"),"actions":[read,write]}));
    }
    let mut manifest = json!({"id":shell.id,"version":version.to_string(),"coreApi":"1","runtime":"declarative","category":"operations","name":brief.name,"permissions":["data.read","data.write","admin.slot"],"entities":brief.entities,"settings":brief.settings,"views":views,"surfaces":surfaces,"slots":[],"apiRoutes":[],"intelligence":{"description":brief.summary,"tools":[],"entities":[]}});
    app_generation::default_actions(&mut manifest, &[])?;
    generation_bindings::new_app_assets(&mut manifest)?;
    Ok(json!({"summary":brief.summary,"manifest":manifest}))
}
#[cfg(test)]
/// The untyped implementation this module replaced, kept as the oracle for the JSON equivalence test.
mod legacy {
    use super::{Brief, empty};
    use crate::domain::{app_generation, generation_bindings, generation_fields};
    use serde_json::{Value, json};
    use veyndo_core::{Result, bad};
    /// The bounded brief a model fills in for a new app, in the languages of the shop.
    pub fn schema(locales: &[String]) -> Value {
        let base = app_generation::schema_for(locales);
        let props = &base["properties"]["manifest"]["properties"];
        let mut entities = props["entities"].clone();
        entities["minItems"] = json!(1);
        entities["maxItems"] = json!(3);
        entities["items"]["properties"]["publicRead"] = json!({"type":"boolean","enum":[false]});
        entities["items"]["properties"]["extends"] = json!({"type":"null"});
        entities["items"]["properties"]["fields"]["maxItems"] = json!(16);
        entities["items"]["properties"]["fields"]["items"] =
            generation_fields::schema(&entities["items"]["properties"]["fields"]["items"]);
        let mut definition = json!({"type":"object","additionalProperties":false,
        "required":["name","entities","settings","placements"],"properties":{
        "name":props["name"],"summary":base["properties"]["summary"],"entities":entities,"settings":props["settings"],
        "placements":{"type":"array","minItems":1,"maxItems":6,"items":{"type":"object","additionalProperties":false,
            "required":["location","entity","label","saveLabel"],"properties":{
            "location":{"type":"string","enum":["admin.navigation","admin.product.tab","admin.customer","admin.order","admin.b2b.company","admin.b2b.unit"]},
            "entity":{"type":"string"},"label":props["name"],"saveLabel":props["name"]}}}}});
        definition["properties"]
            .as_object_mut()
            .unwrap()
            .remove("summary");
        json!({"type":"object","additionalProperties":false,"required":["summary","manifest"],"properties":{"summary":base["properties"]["summary"],"manifest":definition}})
    }
    /// Compiles a brief into a complete native manifest; only an empty private draft accepts it.
    pub fn apply(old: &Value, output: &Value, versions: &Value) -> Result<Value> {
        if !empty(old) {
            return Err(bad("Guided creation requires an empty private draft"));
        }
        if output.as_object().is_none_or(|o| {
            o.len() != 2 || !o.contains_key("summary") || !o.contains_key("manifest")
        }) {
            return Err(bad("Invalid guided app brief envelope"));
        }
        let mut definition = output["manifest"].clone();
        definition
            .as_object_mut()
            .ok_or(bad("Invalid guided app definition"))?
            .insert("summary".into(), output["summary"].clone());
        let brief: Brief =
            serde_json::from_value(definition).map_err(|_| bad("Invalid guided app brief"))?;
        if brief.entities.is_empty()
            || brief.entities.len() > 3
            || brief.settings.len() > 16
            || brief.placements.is_empty()
            || brief.placements.len() > 6
            || brief.entities.iter().any(|e| {
                e["publicRead"] != false
                    || e.get("extends").is_some_and(|v| !v.is_null())
                    || e["fields"].as_array().is_none_or(|f| f.len() > 16)
            })
        {
            return Err(bad(
                "Guided apps need bounded private models and placements",
            ));
        }
        let mut version = semver::Version::parse(old["version"].as_str().unwrap_or("0.1.0"))
            .map_err(|_| bad("Invalid draft version"))?;
        while versions
            .as_array()
            .into_iter()
            .flatten()
            .any(|v| v["app"] == old["id"] && v["version"] == version.to_string())
        {
            version.patch = version
                .patch
                .checked_add(1)
                .ok_or(bad("App version limit exceeded"))?;
        }
        let mut views = Vec::new();
        let mut surfaces = Vec::new();
        for (i, p) in brief.placements.iter().enumerate() {
            let entity = brief
                .entities
                .iter()
                .find(|e| e["name"] == p.entity)
                .ok_or(bad("Placement needs an existing data model"))?;
            let binding = match p.location.as_str() {
                "admin.navigation" => None,
                "admin.product.tab" => Some(("product", "productId")),
                "admin.customer" => Some(("customer", "customerId")),
                "admin.order" => Some(("order", "orderId")),
                "admin.b2b.company" => Some(("b2bCompany", "companyId")),
                "admin.b2b.unit" => Some(("b2bUnit", "unitId")),
                _ => {
                    return Err(bad(
                        "Guided app placement must be a registered private admin location",
                    ));
                }
            };
            let context = if let Some((core, key)) = binding {
                let field = entity["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|f| {
                        f["coreReference"] == core && f["indexed"] == true && f["kind"] == "string"
                    })
                    .ok_or(bad(
                        "Contextual placement needs an indexed native reference field",
                    ))?;
                json!({"field":field["name"],"key":key})
            } else {
                Value::Null
            };
            let suffix = p.entity.chars().take(27).collect::<String>();
            let read = format!("list_{suffix}");
            let write = format!("save_{suffix}");
            let id = format!("view_{i}");
            views.push(json!({"id":id,"layout":"form","blocks":[
            {"id":"records","kind":"table","title":p.label,"entity":p.entity,"readAction":read,"contextBinding":context,"geometry":{"x":0,"y":0,"w":6,"h":10}},
            {"id":"editor","kind":"form","title":p.label,"entity":p.entity,"readAction":read,"writeAction":write,"contextBinding":context,"geometry":{"x":6,"y":0,"w":6,"h":10}},
            {"id":"save","kind":"button","title":p.save_label,"geometry":{"x":6,"y":10,"w":6,"h":2},"handlers":{"click":[{"op":"validate","target":"editor"},{"op":"call","action":write,"input":{"kind":"record","block":"editor"}},{"op":"refresh","target":"records"}]}}
        ]}));
            surfaces.push(json!({"id":id,"location":p.location,"label":p.label,"uiPath":format!("native/{id}"),"actions":[read,write]}));
        }
        let mut result = json!({"summary":brief.summary,"manifest":{"id":old["id"],"version":version.to_string(),"coreApi":"1","runtime":"declarative","category":"operations","name":brief.name,"permissions":["data.read","data.write","admin.slot"],"entities":brief.entities,"settings":brief.settings,"views":views,"surfaces":surfaces,"slots":[],"apiRoutes":[],"intelligence":{"description":brief.summary,"tools":[],"entities":[]}}});
        app_generation::default_actions(&mut result["manifest"], &[])?;
        generation_bindings::new_app_assets(&mut result["manifest"])?;
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::{apply, empty, schema};
    use crate::domain::manifest_validation::validate;
    use crate::domain::{app_generation, generation_bindings};
    use serde_json::{Value, json};
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::Result;
    fn validate_package(m: &Manifest) -> Result<()> {
        validate(m, &json!({}))
    }
    fn original() -> Value {
        json!({"id":"private_app","version":"0.1.0","coreApi":"1","runtime":"declarative","name":{"en":"Test"},"entities":[],"views":[],"surfaces":[],"actions":[],"settings":[]})
    }
    fn brief() -> Value {
        let m: Value = serde_json::from_str(include_str!(
            "../../../../extensions/apps/product-advisor/manifest.json"
        ))
        .unwrap();
        let mut entity = m["entities"][0].clone();
        entity["publicRead"] = json!(false);
        entity["fields"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"design","kind":"file","label":{"en":"Design"}}));
        json!({"summary":m["name"],"manifest":{"name":m["name"],"entities":[entity],"settings":[{"key":"team","kind":"string","label":{"en":"Team"},"required":true}],"placements":[{"location":"admin.product.tab","entity":"entries","label":m["name"],"saveLabel":m["name"]},{"location":"admin.navigation","entity":"entries","label":m["name"],"saveLabel":m["name"]}]}})
    }
    #[test]
    fn merchant_brief_produces_connected_private_views_and_real_native_handlers() {
        assert!(empty(&original()));
        let mut out = apply(&original(), &brief(), &json!([])).unwrap();
        app_generation::default_actions(&mut out["manifest"], &[]).unwrap();
        generation_bindings::complete(&mut out["manifest"]).unwrap();
        let m: Manifest = serde_json::from_value(out["manifest"].clone()).unwrap();
        validate_package(&m).unwrap();
        assert_eq!(m.id, "private_app");
        assert_eq!(m.settings.len(), 1);
        assert_eq!(m.views.len(), 2);
        assert_eq!(m.surfaces[0].location, "admin.product.tab");
        assert_eq!(
            out["manifest"]["views"][0]["blocks"][1]["contextBinding"]["key"],
            "productId"
        );
        assert_eq!(
            out["manifest"]["views"][0]["blocks"][2]["handlers"]["click"][1]["action"],
            "save_entries"
        );
        assert_eq!(
            out["manifest"]["views"][1]["blocks"][1]["contextBinding"],
            Value::Null
        );
        assert!(
            m.actions
                .iter()
                .all(|a| !a.public && !a.mcp.unwrap_or(true))
        );
        let next = apply(
            &original(),
            &brief(),
            &json!([{"app":"private_app","version":"0.1.0"}]),
        )
        .unwrap();
        assert_eq!(next["manifest"]["version"], "0.1.1");
    }
    #[test]
    fn brief_cannot_replace_existing_apps_or_invent_public_locations() {
        let mut old = original();
        old["entities"] = json!([{"name":"existing"}]);
        assert!(!empty(&old));
        assert!(apply(&old, &brief(), &json!([])).is_err());
        for key in ["settings", "lifecycles"] {
            let mut existing = original();
            existing[key] = json!([{"owned":"contract"}]);
            assert!(!empty(&existing));
        }
        let mut capability = original();
        capability["capability"] = json!({"intent":{"en":"Existing rules"}});
        assert!(!empty(&capability));
        for key in ["product.detail", "checkout.summary", "admin.unknown"] {
            let mut out = brief();
            out["manifest"]["placements"][0]["location"] = json!(key);
            assert!(apply(&original(), &out, &json!([])).is_err());
        }
        let mut out = brief();
        out["manifest"]["placements"][0]["entity"] = json!("foreign");
        assert!(apply(&original(), &out, &json!([])).is_err());
        let mut out = brief();
        out["manifest"]["entities"][0]["publicRead"] = json!(true);
        assert!(apply(&original(), &out, &json!([])).is_err());
    }
    #[test]
    fn brief_schema_requires_settings_placements_and_configured_languages() {
        let s = schema(&["it-IT".into()])["properties"]["manifest"].clone();
        assert_eq!(s["properties"]["name"]["required"], json!(["it-IT"]));
        assert!(
            s["required"]
                .as_array()
                .unwrap()
                .contains(&json!("settings"))
        );
        assert!(s["properties"].get("actions").is_none());
        assert!(s["properties"].get("views").is_none());
        let validation = &s["properties"]["entities"]["items"]["properties"]["fields"]["items"]["anyOf"]
            [0]["properties"]["validation"];
        assert_eq!(validation["additionalProperties"], false);
        assert!(validation["properties"].get("required").is_none());
        assert!(validation["properties"].get("type").is_none());
        assert!(validation["properties"].get("enum").is_none());
        assert_eq!(validation["properties"]["minLength"]["type"], "integer");
        assert_eq!(validation["properties"]["minimum"]["type"], "number");
    }
}
#[cfg(test)]
mod equivalence {
    use serde_json::{Value, json};

    fn brief() -> Value {
        json!({"summary":{"en":"Cases"},"manifest":{"name":{"en":"Cases"},
            "entities":[{"name":"cases","publicRead":false,"extends":null,"fields":[
                {"name":"note","kind":"string","label":{"en":"Note"}},
                {"name":"order_id","kind":"string","coreReference":"order","indexed":true,"required":true,"label":{"en":"Order"}},
                {"name":"product_id","kind":"string","coreReference":"product","indexed":true,"required":true,"label":{"en":"Product"}},
                {"name":"photo","kind":"image","label":{"en":"Photo"}}],"label":{"en":"Cases"}}],
            "settings":[{"key":"team","kind":"string","label":{"en":"Team"}}],
            "placements":[{"location":"admin.order","entity":"cases","label":{"en":"Cases"},"saveLabel":{"en":"Save"}},
                {"location":"admin.product.tab","entity":"cases","label":{"en":"Cases"},"saveLabel":{"en":"Save"}},
                {"location":"admin.navigation","entity":"cases","label":{"en":"Cases"},"saveLabel":{"en":"Save"}}]}})
    }

    #[test]
    fn typed_brief_compiles_the_same_manifest() {
        let shells = [
            json!({"id":"private_app","version":"0.1.0","runtime":"declarative","entities":[]}),
            json!({"id":"private_app","version":3,"runtime":"declarative"}),
            json!({"id":"private_app","version":"bad","runtime":"declarative"}),
            json!({"id":"private_app","runtime":"service"}),
        ];
        let mut answers = vec![brief(), json!({"summary":{}}), json!(["x"])];
        for (pointer, value) in [
            ("/manifest/placements/0/location", json!("admin.customer")),
            ("/manifest/placements/0/entity", json!("foreign")),
            ("/manifest/entities/0/publicRead", json!(true)),
            ("/manifest/entities/0/extends", json!({"object":"order"})),
            ("/manifest/entities/0/fields", json!("none")),
            ("/manifest/settings", json!([])),
        ] {
            let mut answer = brief();
            *answer.pointer_mut(pointer).unwrap() = value;
            answers.push(answer);
        }
        let versions = [
            json!([]),
            json!([{"app":"private_app","version":"0.1.0"},{"app":"private_app","version":"0.1.1"},"x"]),
            json!({"not":"a list"}),
        ];
        let mut compiled = 0;
        for shell in &shells {
            for answer in &answers {
                for stored in &versions {
                    let typed = super::apply(shell, answer, stored);
                    let untyped = super::legacy::apply(shell, answer, stored);
                    match (typed, untyped) {
                        (Ok(a), Ok(b)) => {
                            assert_eq!(a, b, "{shell} {answer}");
                            compiled += 1;
                        }
                        (Err(a), Err(b)) => assert_eq!(a.1, b.1, "{shell} {answer}"),
                        (a, b) => panic!("{shell} {answer}: {:?} vs {:?}", a.is_ok(), b.is_ok()),
                    }
                }
            }
        }
        assert!(compiled > 3, "the fixtures must compile real manifests");
    }

    #[test]
    fn typed_brief_schema_is_unchanged() {
        for locales in [
            vec!["en".to_string()],
            vec!["it-IT".into(), "de".into(), "pt-BR".into()],
        ] {
            assert_eq!(super::schema(&locales), super::legacy::schema(&locales));
        }
    }
}
