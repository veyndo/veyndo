//! Localized variant option values: merchant names per content language for each option value key,
//! kept in the product document (`extra.optionValues`, on the family root) and resolved per request into
//! `extra.optionLabels`. The value key stays the identity for variant matching, swatches and carts.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::localized::Languages;
use veyndo_core::{Result, bad};

/// Group → value key → content language key → name; `None` (or a missing language) inherits the main language.
pub type OptionValues = HashMap<String, HashMap<String, HashMap<String, Option<String>>>>;

/// Groups, values per group and the length of a name; product options themselves allow 40 groups.
const GROUPS: usize = 40;
const VALUES: usize = 200;
const NAME: usize = 200;

/// Bounded names in enabled languages only; an explicit name is never empty (use `null` to inherit).
pub fn validate(values: &OptionValues, languages: Languages) -> Result<()> {
    if values.len() > GROUPS {
        return Err(bad("Too many option groups with names"));
    }
    for (group, names) in values {
        if group.trim().is_empty() || group.len() > 100 || names.len() > VALUES {
            return Err(bad("Invalid option value names"));
        }
        for (value, texts) in names {
            if value.trim().is_empty() || value.len() > NAME || texts.len() > 100 {
                return Err(bad("Invalid option value names"));
            }
            for (language, text) in texts {
                if !languages.allowed(language)
                    || text
                        .as_deref()
                        .is_some_and(|t| t.trim().is_empty() || t.len() > NAME || t.contains('\0'))
                {
                    return Err(bad("Invalid option value name"));
                }
            }
        }
    }
    Ok(())
}

/// Stored names as read: group → value key → content language → name.
type StoredNames = BTreeMap<String, BTreeMap<String, Map<String, Value>>>;

/// The fields of `extra` this module reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OptionTexts {
    #[serde(deserialize_with = "lenient")]
    option_values: Option<StoredNames>,
    #[serde(deserialize_with = "lenient")]
    option_labels: Option<BTreeMap<String, String>>,
}

/// The resolved names `hydrate` writes into `extra`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Resolved {
    option_labels: BTreeMap<String, String>,
}

/// The name of one option value along the locale chain (each locale, then its base language);
/// the value key itself when no language names it. Nothing is written back.
pub fn label<'a>(
    texts: Option<&'a Map<String, Value>>,
    value: &'a str,
    chain: &[String],
) -> &'a str {
    texts
        .and_then(|texts| {
            chain
                .iter()
                .flat_map(|l| [l.as_str(), l.split('-').next().unwrap_or(l)])
                .find_map(|l| texts.get(l).and_then(Value::as_str))
        })
        .unwrap_or(value)
}

/// Writes `extra.optionLabels`: the effective name of each of the product's own option values.
pub fn localize(extra: &mut Value, options: &Value, chain: &[String]) {
    let Some(options) = options.as_object().filter(|o| !o.is_empty()) else {
        return;
    };
    let names = view::<OptionTexts>(extra).option_values.unwrap_or_default();
    let option_labels = options
        .iter()
        .filter_map(|(group, value)| {
            let value = value.as_str()?;
            let texts = names.get(group).and_then(|g| g.get(value));
            Some((group.clone(), label(texts, value, chain).to_owned()))
        })
        .collect();
    if !extra.is_object() {
        *extra = Value::Object(Map::new());
    }
    // `extra` is an object here, so the merge cannot fail.
    let _ = merge(extra, Resolved { option_labels });
}

/// The option values of a product for display ("Camel / L"), using resolved names where present.
pub fn display(extra: &Value, options: &Value) -> String {
    let labels = view::<OptionTexts>(extra).option_labels.unwrap_or_default();
    options
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(group, value)| {
            value
                .as_str()
                .map(|v| labels.get(group).map_or(v, String::as_str))
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

#[cfg(test)]
mod tests {
    use super::{OptionValues, display, localize, validate};
    use serde_json::json;
    use veyndo_core::localized::Languages;

    fn chain(locales: &[&str]) -> Vec<String> {
        locales.iter().map(|l| (*l).to_owned()).collect()
    }

    /// Each value resolves along the chain: requested language, its base, the main language, then the key.
    #[test]
    fn names_inherit_the_main_language_and_never_invent_text() {
        let mut extra = json!({"optionValues":{"color":{
            "camel":{"en":"Camel","de":"Kamel","es":null},
            "warm-gray":{"en":"Warm grey"}}}});
        let options = json!({"color":"camel","size":"L"});
        localize(&mut extra, &options, &chain(&["de-DE", "en-GB"]));
        assert_eq!(extra["optionLabels"], json!({"color":"Kamel","size":"L"}));
        assert_eq!(display(&extra, &options), "Kamel / L");
        localize(&mut extra, &options, &chain(&["es-ES", "en-GB"]));
        assert_eq!(extra["optionLabels"]["color"], "Camel");
        localize(
            &mut extra,
            &json!({"color":"warm-gray"}),
            &chain(&["fr-FR", "en-GB"]),
        );
        assert_eq!(extra["optionLabels"]["color"], "Warm grey");
        // The stored names are untouched: no fallback is saved as a translation.
        assert_eq!(extra["optionValues"]["color"]["camel"]["es"], json!(null));
        let mut plain = json!({});
        localize(&mut plain, &json!({"size":"M"}), &chain(&["de-DE"]));
        assert_eq!(display(&plain, &json!({"size":"M"})), "M");
        assert_eq!(display(&json!({}), &json!({"size":"M"})), "M");
    }

    /// Only enabled languages, bounded and non-empty names; `null` inherits.
    #[test]
    fn names_are_bounded_and_in_enabled_languages() {
        let enabled = vec!["en-GB".to_owned(), "de-DE".to_owned(), "it-IT".to_owned()];
        let languages = Languages {
            main: "en-GB",
            enabled: &enabled,
        };
        let parse = |v: serde_json::Value| serde_json::from_value::<OptionValues>(v).unwrap();
        assert!(
            validate(
                &parse(json!({"color":{"camel":{"de":"Kamel","it":"Cammello","en":null}}})),
                languages
            )
            .is_ok()
        );
        for bad in [
            json!({"color":{"camel":{"fr":"Chameau"}}}),
            json!({"color":{"camel":{"de":""}}}),
            json!({"color":{"camel":{"de":"x".repeat(201)}}}),
            json!({"":{"camel":{"de":"Kamel"}}}),
            json!({"color":{" ":{"de":"Kamel"}}}),
        ] {
            assert!(validate(&parse(bad.clone()), languages).is_err(), "{bad}");
        }
    }
}
