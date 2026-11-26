//! Which content languages of a product already have a name or a description, as the catalogue preparation view shows it.
use crate::types::translation::ProductText;
use veyndo_core::localized::Languages;

/// The canonical name and description stored on the product row itself.
#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub name: &'a str,
    pub description: &'a str,
}

/// The enabled languages in which the field is filled. Like the editor, the row's own texts count
/// for the main language only; other missing fields inherit and are not reported as translated.
pub fn filled_locales(
    product: &str,
    source: Option<Source>,
    texts: &[ProductText],
    language_of: impl Fn(&str) -> Option<String>,
    languages: Languages,
    description: bool,
) -> Vec<String> {
    languages
        .enabled
        .iter()
        .filter(|locale| {
            let translated = language_of(locale)
                .and_then(|language| {
                    texts
                        .iter()
                        .find(|t| t.product_id == product && t.language_id == language)
                })
                .and_then(|t| {
                    if description {
                        t.description.as_deref()
                    } else {
                        t.name.as_deref()
                    }
                });
            translated
                .or_else(|| {
                    (locale.as_str() == languages.main)
                        .then(|| source.map(|s| if description { s.description } else { s.name }))
                        .flatten()
                })
                .is_some_and(|value| !value.trim().is_empty())
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Source, filled_locales};
    use crate::types::translation::ProductText;
    use veyndo_core::localized::Languages;

    #[test]
    fn only_the_main_language_counts_the_source_texts_and_blank_is_not_filled() {
        let enabled: Vec<String> = ["es-ES", "de-DE", "fr-FR"]
            .iter()
            .map(|l| (*l).to_owned())
            .collect();
        let languages = Languages {
            main: "es-ES",
            enabled: &enabled,
        };
        let text = |language: &str, name: Option<&str>| ProductText {
            product_id: "p1".into(),
            language_id: language.into(),
            name: name.map(str::to_owned),
            description: None,
        };
        let texts = [text("id-de", Some("Leuchte")), text("id-fr", Some("  "))];
        let language_of = |locale: &str| Some(format!("id-{}", &locale[..2]));
        let source = Some(Source {
            name: "Lámpara",
            description: "",
        });
        assert_eq!(
            filled_locales("p1", source, &texts, language_of, languages, false),
            ["es-ES", "de-DE"]
        );
        assert!(filled_locales("p1", source, &texts, language_of, languages, true).is_empty());
        assert!(filled_locales("p2", None, &texts, language_of, languages, false).is_empty());
    }
}
