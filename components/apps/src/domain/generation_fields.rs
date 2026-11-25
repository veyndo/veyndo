//! Guided field alternatives express native cross-field rules before inference; native admission remains authoritative.
//! The field schema is a JSON Schema keyword document, addressed by path; native `Manifest` validation types the draft afterwards.
use crate::domain::draft_document::schema_at;
use serde_json::{Value, json};

/// Replaces the keyword at `path` of a JSON Schema node, creating missing nodes like JSON indexing does.
fn set(schema: &mut Value, path: &[&str], value: Value) {
    *schema_at(schema, path).expect("field schema nodes are objects") = value;
}

/// The field schema with one alternative per field family, so cross-field rules hold before inference.
pub fn schema(base: &Value) -> Value {
    let mut field = base.clone();
    set(
        &mut field,
        &["properties", "validation"],
        json!({"type":"object","additionalProperties":false,"properties":{
            "minimum":{"type":"number"},"maximum":{"type":"number"},
            "minLength":{"type":"integer","minimum":0},"maxLength":{"type":"integer","minimum":0},
            "minItems":{"type":"integer","minimum":0},"maxItems":{"type":"integer","minimum":0},
            "uniqueItems":{"type":"boolean"}
        }}),
    );
    let mut core = field.clone();
    set(
        &mut core,
        &["properties", "kind"],
        json!({"type":"string","enum":["string"]}),
    );
    set(
        &mut core,
        &["properties", "coreReference"],
        json!({"type":"string","enum":["product","customer","order","b2bCompany","b2bUnit"]}),
    );
    for (key, value) in [
        ("required", true),
        ("indexed", true),
        ("translatable", false),
    ] {
        set(
            &mut core,
            &["properties", key],
            json!({"type":"boolean","enum":[value]}),
        );
    }
    set(&mut core, &["properties", "choices", "maxItems"], json!(0));
    let mut local = field.clone();
    set(
        &mut local,
        &["properties", "coreReference"],
        json!({"type":"null"}),
    );
    set(
        &mut local,
        &["properties", "translatable"],
        json!({"type":"boolean","enum":[false]}),
    );
    let mut string = local.clone();
    set(
        &mut string,
        &["properties", "kind"],
        json!({"type":"string","enum":["string"]}),
    );
    set(&mut local, &["properties", "choices", "maxItems"], json!(0));
    let mut scalar = local.clone();
    set(
        &mut scalar,
        &["properties", "kind"],
        json!({"type":"string","enum":["integer","boolean","date","datetime","decimal","image","file"]}),
    );
    for key in ["indexed", "unique"] {
        set(
            &mut local,
            &["properties", key],
            json!({"type":"boolean","enum":[false]}),
        );
    }
    let mut translated = local.clone();
    set(
        &mut translated,
        &["properties", "kind"],
        json!({"type":"string","enum":["string"]}),
    );
    set(
        &mut translated,
        &["properties", "translatable"],
        json!({"type":"boolean","enum":[true]}),
    );
    set(
        &mut local,
        &["properties", "kind"],
        json!({"type":"string","enum":["json","money","richtext"]}),
    );
    json!({"anyOf":[core,string,translated,scalar,local]})
}

#[cfg(test)]
/// The untyped implementation this module replaced, kept as the oracle for the JSON equivalence test.
mod legacy {
    use serde_json::{Value, json};
    /// The field schema with one alternative per field family, so cross-field rules hold before inference.
    pub fn schema(base: &Value) -> Value {
        let mut field = base.clone();
        field["properties"]["validation"] = json!({"type":"object","additionalProperties":false,"properties":{
            "minimum":{"type":"number"},"maximum":{"type":"number"},
            "minLength":{"type":"integer","minimum":0},"maxLength":{"type":"integer","minimum":0},
            "minItems":{"type":"integer","minimum":0},"maxItems":{"type":"integer","minimum":0},
            "uniqueItems":{"type":"boolean"}
        }});
        let mut core = field.clone();
        core["properties"]["kind"] = json!({"type":"string","enum":["string"]});
        core["properties"]["coreReference"] =
            json!({"type":"string","enum":["product","customer","order","b2bCompany","b2bUnit"]});
        for (key, value) in [
            ("required", true),
            ("indexed", true),
            ("translatable", false),
        ] {
            core["properties"][key] = json!({"type":"boolean","enum":[value]});
        }
        core["properties"]["choices"]["maxItems"] = json!(0);
        let mut local = field.clone();
        local["properties"]["coreReference"] = json!({"type":"null"});
        local["properties"]["translatable"] = json!({"type":"boolean","enum":[false]});
        let mut string = local.clone();
        string["properties"]["kind"] = json!({"type":"string","enum":["string"]});
        local["properties"]["choices"]["maxItems"] = json!(0);
        let mut scalar = local.clone();
        scalar["properties"]["kind"] = json!({"type":"string","enum":["integer","boolean","date","datetime","decimal","image","file"]});
        for key in ["indexed", "unique"] {
            local["properties"][key] = json!({"type":"boolean","enum":[false]});
        }
        let mut translated = local.clone();
        translated["properties"]["kind"] = json!({"type":"string","enum":["string"]});
        translated["properties"]["translatable"] = json!({"type":"boolean","enum":[true]});
        local["properties"]["kind"] = json!({"type":"string","enum":["json","money","richtext"]});
        json!({"anyOf":[core,string,translated,scalar,local]})
    }
}
#[cfg(test)]
mod tests {
    use crate::domain::generation_brief;
    use crate::domain::manifest_validation::validate;
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::Result;
    fn validate_package(m: &Manifest) -> Result<()> {
        validate(m, &json!({}))
    }
    #[test]
    fn every_guided_field_family_compiles_under_actual_native_admission() {
        let schema = generation_brief::schema(&["en-GB".into()]);
        let families = &schema["properties"]["manifest"]["properties"]["entities"]["items"]["properties"]
            ["fields"]["items"]["anyOf"];
        assert_eq!(families.as_array().unwrap().len(), 5);
        for family in families.as_array().unwrap() {
            let p = &family["properties"];
            for kind in p["kind"]["enum"].as_array().unwrap() {
                let mut field = json!({"name":"value","label":{"en-GB":"Value"},"kind":kind,"required":true,"indexed":false,"unique":false,"translatable":false,"coreReference":null,"choices":[],"validation":{}});
                for key in ["indexed", "translatable", "required", "coreReference"] {
                    if let Some(v) = p[key]["enum"].as_array().and_then(|v| v.first()) {
                        field[key] = v.clone();
                    }
                }
                let m: Manifest = serde_json::from_value(json!({"id":"guided_fields","version":"0.1.0","coreApi":"1","runtime":"declarative","name":{"en-GB":"Fields"},"permissions":["data.read","data.write"],"entities":[{"name":"entries","label":{"en-GB":"Entries"},"publicRead":false,"fields":[field]}]})).unwrap();
                validate_package(&m).unwrap();
            }
        }
    }
}
#[cfg(test)]
mod equivalence {
    use serde_json::json;

    #[test]
    fn typed_field_schema_is_unchanged() {
        let item = crate::domain::draft_document::schema_node(
            &crate::domain::app_generation::schema_for(&["en".into()]),
            &[
                "properties",
                "manifest",
                "properties",
                "entities",
                "items",
                "properties",
                "fields",
                "items",
            ],
        );
        for base in [
            item,
            json!({}),
            json!({"properties":{"choices":{"type":"array"}}}),
        ] {
            assert_eq!(super::schema(&base), super::legacy::schema(&base));
        }
    }
}
