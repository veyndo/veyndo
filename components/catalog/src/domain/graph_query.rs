//! Saved category predicates select only current confirmed public source claims; no inference at browse time.
use serde::Deserialize;
use serde_json::{Map, Value};
use veyndo_core::document::{lenient, view};
use veyndo_core::localized::Languages;
use veyndo_core::{Result, bad};

pub const TYPES: &[&str] = &[
    "intent", "problem", "occasion", "audience", "material", "property",
];

/// The JSON fields `validate` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateInput {
    r#type: Value,
}

/// The fields of a saved fact query, typed as `validate` reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FactQuery {
    #[serde(deserialize_with = "lenient")]
    node_type: Option<String>,
    #[serde(deserialize_with = "lenient")]
    minimum_confidence: Option<f64>,
    #[serde(deserialize_with = "lenient")]
    text: Option<Map<String, Value>>,
}

/// A saved fact query belongs to a listing, names a supported node type and has text in the main language.
pub fn validate(data: &Value, languages: Languages) -> Result<()> {
    let input: ValidateInput = view(data);
    let Some(query) = data.get("graphQuery").filter(|v| !v.is_null()) else {
        return Ok(());
    };
    let object = query.as_object().ok_or(bad("Invalid saved fact query"))?;
    let fact: FactQuery = view(query);
    if input.r#type != "page"
        || object.len() != 3
        || object
            .keys()
            .any(|key| !["nodeType", "minimumConfidence", "text"].contains(&key.as_str()))
        || !TYPES.contains(&fact.node_type.as_deref().unwrap_or(""))
        || !fact
            .minimum_confidence
            .is_some_and(|n| n.is_finite() && (0.0..=1.0).contains(&n))
    {
        return Err(bad(
            "Saved fact query requires a listing, supported node type and confidence 0..1",
        ));
    }
    let text = fact
        .text
        .as_ref()
        .filter(|m| !m.is_empty() && m.len() <= 100)
        .ok_or(bad("Localized fact query required"))?;
    for (locale, value) in text {
        if !languages.allowed(locale)
            || (!value.is_null()
                && !value.as_str().is_some_and(|s| {
                    s.is_empty() || (s.trim() == s && s.chars().count() >= 3 && s.len() <= 120)
                }))
        {
            return Err(bad(
                "Fact query text requires an enabled language and 3..120 bytes, or explicit empty/inherit",
            ));
        }
    }
    let main = text
        .get(languages.main)
        .and_then(Value::as_str)
        .or_else(|| {
            text.get(languages.main.split('-').next().unwrap_or(""))
                .and_then(Value::as_str)
        })
        .unwrap_or("");
    if main.is_empty() {
        return Err(bad("Main-language fact query required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use serde_json::{Value, json};
    use veyndo_core::localized::Languages;
    #[test]
    fn saved_query_is_closed_localized_and_bounded() {
        let enabled: Vec<String> = ["en-GB", "de-DE", "fr-FR", "es-ES"]
            .iter()
            .map(|l| (*l).to_owned())
            .collect();
        let languages = Languages {
            main: "en-GB",
            enabled: &enabled,
        };
        let mut data = json!({"type":"page","graphQuery":{"nodeType":"intent","minimumConfidence":0.8,"text":{"en":"cycling","de":null,"es":""}}});
        assert!(validate(&data, languages).is_ok());
        for q in [
            json!({"nodeType":"unknown","minimumConfidence":0.8,"text":{"en":"cycling"}}),
            json!({"nodeType":"intent","minimumConfidence":1.1,"text":{"en":"cycling"}}),
            json!({"nodeType":"intent","minimumConfidence":0.8,"text":{"en":"ab"}}),
            json!({"nodeType":"intent","minimumConfidence":0.8,"text":{"en":"cycling","xx":"test"}}),
            json!({"nodeType":"intent","minimumConfidence":0.8,"text":{"en":"cycling"},"sql":"arbitrary"}),
        ] {
            data["graphQuery"] = q;
            assert!(validate(&data, languages).is_err());
        }
        data["graphQuery"] = Value::Null;
        assert!(validate(&data, languages).is_ok());
    }
}
