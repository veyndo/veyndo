//! A translated name and description as stored per content language; a missing field inherits from the shop's main language.
use crate::{Result, bad};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod languages;
pub use languages::{Languages, valid_locale_key};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// The name or description for `locale`: the exact language, its base language, then the shop's main language.
pub fn pick<'a>(
    values: &'a HashMap<String, Text>,
    locale: &str,
    main: &str,
    description: bool,
) -> Option<&'a str> {
    [
        locale,
        locale.split('-').next().unwrap_or(locale),
        main,
        main.split('-').next().unwrap_or(main),
    ]
    .into_iter()
    .find_map(|key| {
        values.get(key).and_then(|t| {
            if description {
                t.description.as_deref()
            } else {
                t.name.as_deref()
            }
        })
    })
}

/// Translated names and descriptions may only use enabled languages and stay within their bounds.
pub fn validate_texts(values: &HashMap<String, Text>, enabled: &[String]) -> Result<()> {
    if values.len() > 100
        || values.iter().any(|(l, t)| {
            !enabled
                .iter()
                .any(|x| x == l || x.split('-').next() == Some(l.as_str()))
                || t.name.as_ref().is_some_and(|v| v.len() > 200)
                || t.description.as_ref().is_some_and(|v| v.len() > 4000)
        })
    {
        return Err(bad("Invalid translated method text"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Text, pick, validate_texts};
    use std::collections::HashMap;

    fn text(name: Option<&str>, description: Option<&str>) -> Text {
        Text {
            name: name.map(str::to_owned),
            description: description.map(str::to_owned),
        }
    }

    #[test]
    fn each_field_inherits_on_its_own_from_the_main_language() {
        let values = HashMap::from([
            ("de".to_owned(), text(Some("Standard"), Some("Zwei Tage"))),
            ("es".to_owned(), text(Some("Estándar"), None)),
        ]);
        assert_eq!(pick(&values, "es-ES", "de-DE", false), Some("Estándar"));
        assert_eq!(pick(&values, "es-ES", "de-DE", true), Some("Zwei Tage"));
        assert_eq!(pick(&values, "fr-FR", "en-GB", false), None);
    }

    #[test]
    fn texts_need_an_enabled_language_and_bounded_length() {
        let enabled = ["de-DE".to_owned(), "es-ES".to_owned()];
        let ok = HashMap::from([("es".to_owned(), text(Some("Estándar"), None))]);
        assert!(validate_texts(&ok, &enabled).is_ok());
        let foreign = HashMap::from([("fr".to_owned(), text(Some("Standard"), None))]);
        assert!(validate_texts(&foreign, &enabled).is_err());
        let long = HashMap::from([("de".to_owned(), text(Some(&"x".repeat(201)), None))]);
        assert!(validate_texts(&long, &enabled).is_err());
    }
}
