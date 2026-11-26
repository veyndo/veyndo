//! Translate only human-readable text; preserve identifiers, URLs, rich structure and source specification keys.
use crate::domain::rich::validate_rich;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};

/// The texts of a product in the source language, as far as they are stored.
pub struct Source {
    pub name: String,
    pub description: String,
    pub translated_name: Option<String>,
    pub translated_description: Option<String>,
    pub extra: Value,
}

/// The translatable document of a product: texts of `key`, falling back to the main language key.
pub fn source(row: Source, key: &str, main: &str) -> Value {
    let pick = |field: &str| {
        row.extra[field]
            .get(key)
            .or_else(|| row.extra[field].get(main))
            .cloned()
            .unwrap_or(Value::Null)
    };
    json!({"name":row.translated_name.unwrap_or(row.name),
        "description":row.translated_description.unwrap_or(row.description),
        "seo":pick("seo"),"specifications":pick("specifications"),"richDescription":pick("richDescription")})
}

/// Writes the translated metadata of one language into a product's `extra`; texts the result leaves out stay as they are.
pub fn merge_metadata(extra: &mut Value, result: &Value, key: &str) -> Result<()> {
    for name in ["seo", "specifications", "richDescription"] {
        if !result[name].is_null() {
            if !extra[name].is_object() {
                extra[name] = json!({});
            }
            extra[name][key] = result[name].clone();
        }
    }
    // Persist through the same safe rich-content validator used by manual edits.
    let merged: Metadata = view(extra);
    if !merged.rich_description.is_null() {
        validate_rich(&merged.rich_description)?;
    }
    Ok(())
}

/// Every human-readable text of a product as (JSON pointer, text); identifiers, URLs and structure are left out.
pub fn texts(value: &Value) -> Vec<(String, String)> {
    fn visit(v: &Value, path: &str, out: &mut Vec<(String, String)>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    let pointer = format!("{path}/{}", k.replace('~', "~0").replace('/', "~1"));
                    if v.is_string()
                        && (matches!(
                            k.as_str(),
                            "name" | "description" | "title" | "text" | "alt"
                        ) || path == "/specifications")
                    {
                        if !v.as_str().unwrap().is_empty() {
                            out.push((pointer, v.as_str().unwrap().into()));
                        }
                    } else {
                        visit(v, &pointer, out);
                    }
                }
            }
            Value::Array(a) => {
                for (i, v) in a.iter().enumerate() {
                    visit(v, &format!("{path}/{i}"), out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    visit(value, "", &mut out);
    out
}
/// One translated text of a model response, addressed by its JSON pointer.
#[derive(Deserialize, Default)]
#[serde(default)]
struct TranslatedEntry {
    #[serde(deserialize_with = "lenient")]
    path: Option<String>,
    #[serde(deserialize_with = "lenient")]
    text: Option<String>,
}

/// The translatable metadata of a product whose limits are checked.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Metadata {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    seo: Seo,
    #[serde(deserialize_with = "lenient")]
    specifications: Option<Map<String, Value>>,
    rich_description: Value,
}

/// The search engine texts of a product.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Seo {
    #[serde(deserialize_with = "lenient")]
    title: Option<String>,
    #[serde(deserialize_with = "lenient")]
    description: Option<String>,
}

/// The JSON fields `translated` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct TranslatedInput {
    translations: Value,
}

/// Applies a model response to the source: exactly the known paths, each once, within the product limits.
/// Anything else is refused, so the model cannot add, drop or restructure fields.
pub fn translated(source: &Value, response: &Value) -> Result<Value> {
    let input: TranslatedInput = view(response);
    let original = texts(source);
    let entries = input
        .translations
        .as_array()
        .ok_or(bad("Translation response missing"))?;
    if entries.len() != original.len() {
        return Err(bad("Incomplete translation response"));
    }
    let entries: Vec<TranslatedEntry> = entries.iter().map(view).collect();
    let mut seen = std::collections::HashSet::new();
    let mut result = source.clone();
    for entry in &entries {
        let path = entry
            .path
            .as_deref()
            .ok_or(bad("Translation path missing"))?;
        let text = entry
            .text
            .as_deref()
            .ok_or(bad("Translation text missing"))?;
        if !seen.insert(path)
            || !original.iter().any(|(p, _)| p == path)
            || text.trim().is_empty()
            || text.len() > 4000
        {
            return Err(bad("Invalid translated field"));
        }
        *result
            .pointer_mut(path)
            .ok_or(bad("Unknown translation field"))? = json!(text);
    }
    let translated: Metadata = view(&result);
    if translated
        .name
        .as_deref()
        .is_none_or(|n| n.is_empty() || n.len() > 200)
        || translated
            .seo
            .title
            .as_deref()
            .is_some_and(|s| s.len() > 200)
        || translated
            .seo
            .description
            .as_deref()
            .is_some_and(|s| s.len() > 500)
        || translated.specifications.as_ref().is_some_and(|m| {
            m.values()
                .any(|v| v.as_str().is_some_and(|s| s.len() > 1000))
        })
    {
        return Err(bad("Translated metadata exceeds product limits"));
    }
    if !translated.rich_description.is_null() {
        validate_rich(&json!({"en":translated.rich_description}))?;
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::{Source, source, texts, translated};
    use serde_json::json;
    #[test]
    fn preserve_links_and_refuse_model_structure_changes() {
        let s = json!({"name":"Desk","description":"Wood","richDescription":[{"type":"image","url":"https://example.com/a.png","alt":"Desk"},{"type":"document","doc":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Hello","marks":[{"type":"link","attrs":{"href":"https://example.com"}}]}]}]}}]});
        let entries = texts(&s)
            .iter()
            .map(|(p, t)| json!({"path":p,"text":format!("ES {t}")}))
            .collect::<Vec<_>>();
        let r = translated(&s, &json!({"translations":entries})).unwrap();
        assert_eq!(
            r["richDescription"][0]["url"],
            s["richDescription"][0]["url"]
        );
        assert!(translated(&s, &json!({"translations":[{"path":"/price","text":"0"}]})).is_err());
        assert!(translated(&s, &json!({"translations":[]})).is_err());
    }

    #[test]
    fn merged_metadata_keeps_other_languages_and_untouched_fields() {
        use super::merge_metadata;
        let mut extra = json!({"seo":{"en":{"title":"Desk"}},"specifications":"legacy"});
        let result = json!({"seo":{"title":"Escritorio"},"specifications":{"Material":"Madera"},"richDescription":null});
        merge_metadata(&mut extra, &result, "es").unwrap();
        assert_eq!(extra["seo"]["en"]["title"], "Desk");
        assert_eq!(extra["seo"]["es"]["title"], "Escritorio");
        assert_eq!(extra["specifications"]["es"]["Material"], "Madera");
        assert!(extra["richDescription"].is_null());
    }
    #[test]
    fn the_source_document_prefers_stored_translations_and_falls_back_to_main_metadata() {
        let extra = json!({"seo":{"de":{"title":"Tisch"},"en":{"title":"Desk"}},"specifications":{"en":{"Material":"Oak"}}});
        let row = |translated: bool| Source {
            name: "Desk".into(),
            description: "Wood".into(),
            translated_name: translated.then(|| "Tisch".into()),
            translated_description: None,
            extra: extra.clone(),
        };
        let s = source(row(true), "de", "en");
        assert_eq!(s["name"], "Tisch");
        assert_eq!(s["description"], "Wood");
        assert_eq!(s["seo"], json!({"title":"Tisch"}));
        assert_eq!(s["specifications"], json!({"Material":"Oak"}));
        assert!(s["richDescription"].is_null());
        assert_eq!(source(row(false), "fr", "en")["name"], "Desk");
        assert_eq!(
            source(row(false), "fr", "en")["seo"],
            json!({"title":"Desk"})
        );
    }
}
