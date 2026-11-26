//! Translated product hydration: each field is resolved on its own along the language chain.
use crate::domain::content::localize_extra;
use crate::types::translation::ProductText;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use veyndo_core::document::{lenient, view};
use veyndo_pricing::catalog::Product;

/// Hydrate locked rows using the same field-level inheritance as catalog reads.
pub fn hydrate(
    mut ps: Vec<Product>,
    texts: &[ProductText],
    chain: &[String],
    locale_chain: &[String],
) -> Vec<Product> {
    let mut translations: HashMap<String, ProductTranslations> = HashMap::new();
    for text in texts {
        translations
            .entry(text.product_id.clone())
            .or_default()
            .insert(
                text.language_id.clone(),
                (text.name.clone(), text.description.clone()),
            );
    }
    for p in &mut ps {
        localize_extra(&mut p.extra, locale_chain);
        if let Some(media) = p.media.as_array_mut() {
            for m in media {
                localize_media(m, locale_chain);
            }
        }
        if let Some(values) = translations.get(&p.id) {
            if let Some(name) = translated_field(values, chain, false) {
                p.name = name.to_owned();
            }
            if let Some(description) = translated_field(values, chain, true) {
                p.description = description.to_owned();
            }
        }
    }
    ps
}
type ProductTranslations = HashMap<String, (Option<String>, Option<String>)>;

fn translated_field<'a>(
    values: &'a ProductTranslations,
    chain: &[String],
    description: bool,
) -> Option<&'a str> {
    // Resolve each field independently: NULL falls back, an empty string does not.
    chain.iter().find_map(|id| {
        values.get(id).and_then(|(name, text)| {
            if description {
                text.as_deref()
            } else {
                name.as_deref()
            }
        })
    })
}

/// The localized texts of a media item.
#[derive(Deserialize, Default)]
#[serde(default)]
struct MediaTexts {
    #[serde(deserialize_with = "lenient")]
    caption: Option<Map<String, Value>>,
    #[serde(deserialize_with = "lenient")]
    alt: Option<Map<String, Value>>,
}

/// The first text along the locale chain, trying each locale and then its language.
fn along_chain<'a>(texts: &'a Map<String, Value>, locale_chain: &[String]) -> Option<&'a str> {
    locale_chain
        .iter()
        .flat_map(|l| [l.as_str(), l.split('-').next().unwrap_or(l)])
        .find_map(|l| texts.get(l).and_then(Value::as_str))
}

/// Replaces a localized caption by its text along the chain, and takes the localized alt text as the media `view`.
fn localize_media(m: &mut Value, locale_chain: &[String]) {
    let texts: MediaTexts = view(m);
    let Some(item) = m.as_object_mut() else {
        return;
    };
    if let Some(caption) = &texts.caption {
        let text = along_chain(caption, locale_chain).unwrap_or("");
        item.insert("caption".into(), json!(text));
    }
    if let Some(text) = texts
        .alt
        .as_ref()
        .and_then(|alt| along_chain(alt, locale_chain))
    {
        item.insert("view".into(), json!(text));
    }
}

#[cfg(test)]
mod tests {
    use super::{hydrate, translated_field};
    use crate::types::translation::ProductText;
    use serde_json::json;
    use std::collections::HashMap;
    use veyndo_pricing::catalog::Product;

    #[test]
    fn indexed_translations_preserve_field_fallback_and_empty_values() {
        let values = HashMap::from([
            ("child".into(), (Some(String::new()), None)),
            (
                "parent".into(),
                (
                    Some("Parent name".into()),
                    Some("Parent description".into()),
                ),
            ),
        ]);
        let chain = vec!["missing".into(), "child".into(), "parent".into()];
        assert_eq!(translated_field(&values, &chain, false), Some(""));
        assert_eq!(
            translated_field(&values, &chain, true),
            Some("Parent description")
        );
        assert_eq!(translated_field(&values, &["absent".into()], false), None);
    }

    #[test]
    fn name_description_and_image_texts_follow_the_language_chain() {
        let product: Product = serde_json::from_value(json!({
            "extra":{"seo":{"es":{"title":"Lámpara"}}},"id":"p1","name":"Source","category":"",
            "description":"Source text","price":1.0,"tax_rate":19.0,"stock":1,"revision":1,
            "list_price":null,"regulation_price":null,"reference_price":null,"advanced_prices":[],
            "min_purchase":1,"purchase_steps":1,"max_purchase":null,"parent_id":null,"options":{},
            "media":[{"url":"/media/a","view":"front","alt":{"es":"Lámpara de mesa"},"caption":{"de":"Tischleuchte"}}],
            "properties":{},"delivery_days":1
        }))
        .unwrap();
        let text = |language: &str, name: Option<&str>, description: Option<&str>| ProductText {
            product_id: "p1".into(),
            language_id: language.into(),
            name: name.map(str::to_owned),
            description: description.map(str::to_owned),
        };
        let texts = [
            text("de", None, Some("")),
            text("es", Some("Lámpara"), Some("Luz")),
        ];
        let chain = vec!["de".to_owned(), "es".to_owned()];
        let locales = vec!["de-DE".to_owned(), "es-ES".to_owned()];
        let out = hydrate(vec![product], &texts, &chain, &locales);
        assert_eq!(out[0].name, "Lámpara");
        assert_eq!(out[0].description, "");
        assert_eq!(out[0].media[0]["caption"], "Tischleuchte");
        assert_eq!(out[0].media[0]["view"], "Lámpara de mesa");
        assert_eq!(out[0].extra["seo"]["de"]["title"], "Lámpara");
    }
}
