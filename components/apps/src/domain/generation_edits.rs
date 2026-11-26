//! Bounded RFC 6902 edits modify the canonical Manifest; this is model wire adaptation, not an app runtime.
//! The edit answer is typed; the edited manifest stays open JSON because RFC 6902 paths address arbitrary
//! app-declared keys.
use crate::domain::draft_document::{elements, schema_node, write};
use crate::domain::generation_brief::VersionRow;
use crate::domain::{app_generation, generation_contract};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};

/// The system prompt for editing an existing manifest in the languages of the shop.
pub fn system(locales: &[String]) -> String {
    let schema = app_generation::schema_for(locales);
    let member = |key: &str| schema_node(&schema, &["properties", "manifest", "properties", key]);
    format!(
        include_str!("../../../../crates/cognition/prompts/app-edit.v3.txt"),
        member("entities"),
        member("views"),
        member("surfaces"),
        member("lifecycles")
    )
}

/// The answer a model gives for an edit: a summary and a bounded list of patch operations.
pub fn schema(locales: &[String]) -> Value {
    let summary = schema_node(
        &app_generation::schema_for(locales),
        &["properties", "summary"],
    );
    json!({"type":"object","additionalProperties":false,"required":["summary","patch"],"properties":{
    "summary":summary,"patch":{"type":"array","minItems":1,"maxItems":64,"items":{
        "type":"object","additionalProperties":false,"required":["op","path","valueJson"],"properties":{
            "op":{"type":"string","enum":["add","replace"]},"path":{"type":"string"},
            "valueJson":{"type":"string","description":"JSON-encode the exact replacement value, including quotes for a string. The host decodes this into the RFC 6902 value."}
        }}}}})
}

/// The edit answer as the host reads it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct EditAnswer {
    summary: Value,
    patch: Value,
}

/// One proposed edit; each member reads as `None` when it is not a string.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ProposedEdit {
    #[serde(deserialize_with = "lenient")]
    op: Option<String>,
    #[serde(deserialize_with = "lenient")]
    path: Option<String>,
    #[serde(deserialize_with = "lenient")]
    value_json: Option<String>,
}

/// The manifest members the version search reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Current {
    id: Value,
    #[serde(deserialize_with = "lenient")]
    version: Option<String>,
}

#[derive(Serialize)]
struct NextVersion {
    version: String,
}

/// Applies a bounded patch to the canonical manifest and keeps existing storage and access boundaries.
pub fn apply(old: &Value, output: &Value, versions: &Value) -> Result<Value> {
    let answer: EditAnswer = view(output);
    let edits = answer
        .patch
        .as_array()
        .filter(|p| !p.is_empty() && p.len() <= 64)
        .ok_or(bad("AI must propose between 1 and 64 app edits"))?;
    if output.to_string().len() > 65536 {
        return Err(bad("AI app edits exceed 64 KiB"));
    }
    let mut operations = Vec::new();
    for edit in edits {
        let edit: ProposedEdit = view(edit);
        let path = edit
            .path
            .as_deref()
            .ok_or(bad("AI edit needs a JSON pointer"))?;
        let root = path
            .strip_prefix('/')
            .and_then(|s| s.split('/').next())
            .unwrap_or("");
        if ![
            "entities",
            "views",
            "surfaces",
            "name",
            "presentation",
            "permissions",
            "apiRoutes",
            "intelligence",
            "slots",
            "lifecycles",
        ]
        .contains(&root)
            || !matches!(edit.op.as_deref(), Some("add" | "replace"))
        {
            return Err(bad(
                "AI edit targets protected app metadata or an unsupported operation",
            ));
        }
        let encoded = edit
            .value_json
            .as_deref()
            .ok_or(bad("AI edit needs JSON-encoded value"))?;
        let value: Value =
            serde_json::from_str(encoded).map_err(|_| bad("AI edit contains invalid JSON"))?;
        operations.push(json!({"op":edit.op,"path":path,"value":value}));
    }
    let patch: json_patch::Patch =
        serde_json::from_value(json!(operations)).map_err(|_| bad("Invalid AI JSON patch"))?;
    let mut manifest = old.clone();
    json_patch::patch(&mut manifest, &patch)
        .map_err(|_| bad("AI edit does not match the current app; refine the instruction"))?;
    generation_contract::retain(old, &mut manifest)?;
    let mut baseline = old.clone();
    generation_contract::retain(old, &mut baseline)?;
    if manifest == baseline {
        return Err(bad(
            "AI proposed no effective app changes; refine the instruction",
        ));
    }
    if manifest.to_string().len() > 65536 {
        return Err(bad("Edited app exceeds 64 KiB"));
    }
    let current: Current = view(old);
    let mut version = semver::Version::parse(current.version.as_deref().unwrap_or(""))
        .map_err(|_| bad("Current app requires a semantic version"))?;
    version.pre = semver::Prerelease::EMPTY;
    version.build = semver::BuildMetadata::EMPTY;
    let stored: Vec<VersionRow> = elements(versions);
    loop {
        version.patch = version
            .patch
            .checked_add(1)
            .ok_or(bad("App version overflow"))?;
        if !stored
            .iter()
            .any(|v| v.app == current.id && v.version == version.to_string())
        {
            break;
        }
    }
    write(
        &mut manifest,
        NextVersion {
            version: version.to_string(),
        },
    )?;
    Ok(json!({"manifest":manifest,"summary":answer.summary}))
}

#[cfg(test)]
/// The untyped implementation this module replaced, kept as the oracle for the JSON equivalence test.
mod legacy {
    use crate::domain::{app_generation, generation_contract};
    use serde_json::{Value, json};
    use veyndo_core::{Result, bad};

    /// The system prompt for editing an existing manifest in the languages of the shop.
    pub fn system(locales: &[String]) -> String {
        let manifest = app_generation::schema_for(locales)["properties"]["manifest"].clone();
        format!(
            include_str!("../../../../crates/cognition/prompts/app-edit.v3.txt"),
            manifest["properties"]["entities"],
            manifest["properties"]["views"],
            manifest["properties"]["surfaces"],
            manifest["properties"]["lifecycles"]
        )
    }

    /// The answer a model gives for an edit: a summary and a bounded list of patch operations.
    pub fn schema(locales: &[String]) -> Value {
        let summary = app_generation::schema_for(locales)["properties"]["summary"].clone();
        json!({"type":"object","additionalProperties":false,"required":["summary","patch"],"properties":{
        "summary":summary,"patch":{"type":"array","minItems":1,"maxItems":64,"items":{
            "type":"object","additionalProperties":false,"required":["op","path","valueJson"],"properties":{
                "op":{"type":"string","enum":["add","replace"]},"path":{"type":"string"},
                "valueJson":{"type":"string","description":"JSON-encode the exact replacement value, including quotes for a string. The host decodes this into the RFC 6902 value."}
            }}}}})
    }

    /// Applies a bounded patch to the canonical manifest and keeps existing storage and access boundaries.
    pub fn apply(old: &Value, output: &Value, versions: &Value) -> Result<Value> {
        let edits = output["patch"]
            .as_array()
            .filter(|p| !p.is_empty() && p.len() <= 64)
            .ok_or(bad("AI must propose between 1 and 64 app edits"))?;
        if output.to_string().len() > 65536 {
            return Err(bad("AI app edits exceed 64 KiB"));
        }
        let mut operations = Vec::new();
        for edit in edits {
            let path = edit["path"]
                .as_str()
                .ok_or(bad("AI edit needs a JSON pointer"))?;
            let root = path
                .strip_prefix('/')
                .and_then(|s| s.split('/').next())
                .unwrap_or("");
            if ![
                "entities",
                "views",
                "surfaces",
                "name",
                "presentation",
                "permissions",
                "apiRoutes",
                "intelligence",
                "slots",
                "lifecycles",
            ]
            .contains(&root)
                || !matches!(edit["op"].as_str(), Some("add" | "replace"))
            {
                return Err(bad(
                    "AI edit targets protected app metadata or an unsupported operation",
                ));
            }
            let encoded = edit["valueJson"]
                .as_str()
                .ok_or(bad("AI edit needs JSON-encoded value"))?;
            let value: Value =
                serde_json::from_str(encoded).map_err(|_| bad("AI edit contains invalid JSON"))?;
            operations.push(json!({"op":edit["op"],"path":path,"value":value}));
        }
        let patch: json_patch::Patch =
            serde_json::from_value(json!(operations)).map_err(|_| bad("Invalid AI JSON patch"))?;
        let mut manifest = old.clone();
        json_patch::patch(&mut manifest, &patch)
            .map_err(|_| bad("AI edit does not match the current app; refine the instruction"))?;
        generation_contract::retain(old, &mut manifest)?;
        let mut baseline = old.clone();
        generation_contract::retain(old, &mut baseline)?;
        if manifest == baseline {
            return Err(bad(
                "AI proposed no effective app changes; refine the instruction",
            ));
        }
        if manifest.to_string().len() > 65536 {
            return Err(bad("Edited app exceeds 64 KiB"));
        }
        let mut version = semver::Version::parse(old["version"].as_str().unwrap_or(""))
            .map_err(|_| bad("Current app requires a semantic version"))?;
        version.pre = semver::Prerelease::EMPTY;
        version.build = semver::BuildMetadata::EMPTY;
        loop {
            version.patch = version
                .patch
                .checked_add(1)
                .ok_or(bad("App version overflow"))?;
            if !versions
                .as_array()
                .into_iter()
                .flatten()
                .any(|v| v["app"] == old["id"] && v["version"] == version.to_string())
            {
                break;
            }
        }
        manifest["version"] = json!(version.to_string());
        Ok(json!({"manifest":manifest,"summary":output["summary"]}))
    }
}
#[cfg(test)]
mod tests {
    use super::{apply, schema, system};
    use serde_json::{Value, json};
    #[test]
    fn the_edit_contract_is_bounded_and_speaks_the_shop_languages() {
        let locales = ["it-IT".to_string()];
        let prompt = system(&locales);
        assert!(prompt.starts_with("[prompt:app-edit@3]"));
        assert!(prompt.contains("it-IT"));
        assert!(prompt.contains("\"entity\""));
        let answer = schema(&locales);
        assert_eq!(answer["required"], json!(["summary", "patch"]));
        assert_eq!(answer["properties"]["patch"]["minItems"], 1);
        assert_eq!(answer["properties"]["patch"]["maxItems"], 64);
        let operation = &answer["properties"]["patch"]["items"];
        assert_eq!(
            operation["properties"]["op"]["enum"],
            json!(["add", "replace"])
        );
        assert_eq!(operation["additionalProperties"], false);
        assert!(
            answer["properties"]["summary"]
                .to_string()
                .contains("it-IT")
        );
    }
    fn original() -> Value {
        json!({"id":"test_app","version":"1.0.0","entities":[{"name":"notes","publicRead":false,"fields":[{"name":"text","kind":"string"}]}],"views":[{"id":"main","blocks":[]}],"events":["order.placed"]})
    }
    fn edit(path: &str, value: Value) -> Value {
        json!({"summary":{"en":"Edit"},"patch":[{"op":"add","path":path,"valueJson":value.to_string()}]})
    }
    #[test]
    fn appends_native_control_without_rewriting_storage_and_skips_used_version() {
        let old = original();
        let out = apply(
            &old,
            &edit(
                "/views/0/blocks/-",
                json!({"id":"notes","kind":"textbox","entity":"notes","dataField":"text"}),
            ),
            &json!([{"app":"test_app","version":"1.0.1"}]),
        )
        .unwrap();
        assert_eq!(out["manifest"]["version"], "1.0.2");
        assert_eq!(out["manifest"]["entities"], old["entities"]);
        assert_eq!(out["manifest"]["events"], old["events"]);
        assert_eq!(out["manifest"]["views"][0]["blocks"][0]["kind"], "textbox");
        assert_eq!(old, original());
    }
    #[test]
    fn rejects_metadata_changes_destructive_schemas_invalid_paths_and_noops() {
        for output in [
            edit("/id", json!("foreign")),
            edit("/version", json!("9.0.0")),
            edit("/entities", json!([])),
            edit("/entities/0/fields/0/kind", json!("integer")),
            edit("/views/99/blocks/-", json!({})),
            edit("/views", original()["views"].clone()),
            json!({"patch":[]}),
        ] {
            assert!(apply(&original(), &output, &json!([])).is_err());
        }
    }
}
#[cfg(test)]
mod equivalence {
    use serde_json::{Value, json};

    #[test]
    fn typed_edits_produce_the_same_manifest() {
        let old = json!({"id":"app","version":"1.2.3-beta+7","name":{"en":"Old"},"entities":[],"views":[],"surfaces":[]});
        let edit =
            |op: Value, path: Value, value: &str| json!({"op":op,"path":path,"valueJson":value});
        let answers = [
            json!({"summary":{"en":"Rename"},"patch":[edit(json!("replace"),json!("/name"),r#"{"en":"New"}"#)]}),
            json!({"summary":{"en":"Add"},"patch":[edit(json!("add"),json!("/slots"),"[]")]}),
            json!({"summary":null,"patch":[edit(json!("remove"),json!("/name"),"1")]}),
            json!({"summary":{},"patch":[edit(json!(7),json!("/name"),"1")]}),
            json!({"summary":{},"patch":[edit(json!("replace"),json!(1),"1")]}),
            json!({"summary":{},"patch":[edit(json!("replace"),json!("/id"),"\"x\"")]}),
            json!({"summary":{},"patch":[edit(json!("replace"),json!("/name"),"{")]}),
            json!({"summary":{},"patch":[edit(json!("replace"),json!("/name"),r#"{"en":"Old"}"#)]}),
            json!({"summary":{},"patch":[]}),
            json!({"summary":{},"patch":"none"}),
            json!(null),
        ];
        let versions = [
            json!([]),
            json!([{"app":"app","version":"1.2.4"},"x"]),
            json!(null),
        ];
        let mut edited = 0;
        for answer in &answers {
            for stored in &versions {
                for current in [old.clone(), json!({"id":"app","version":5,"name":{}})] {
                    match (
                        super::apply(&current, answer, stored),
                        super::legacy::apply(&current, answer, stored),
                    ) {
                        (Ok(a), Ok(b)) => {
                            assert_eq!(a, b, "{answer}");
                            edited += 1;
                        }
                        (Err(a), Err(b)) => assert_eq!(a.1, b.1, "{answer}"),
                        (a, b) => panic!("{answer}: {:?} vs {:?}", a.is_ok(), b.is_ok()),
                    }
                }
            }
        }
        assert!(edited >= 6);
    }

    #[test]
    fn typed_edit_prompt_and_schema_are_unchanged() {
        let locales = ["en".to_string(), "es-MX".into()];
        assert_eq!(super::system(&locales), super::legacy::system(&locales));
        assert_eq!(super::schema(&locales), super::legacy::schema(&locales));
    }
}
