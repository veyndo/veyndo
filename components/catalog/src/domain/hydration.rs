//! Translated product hydration: each field is resolved on its own along the language chain.
use crate::domain::content::localize_extra;
use crate::types::translation::ProductText;
use serde_json::json;
use std::collections::HashMap;
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
                if m["caption"].is_object() {
                    m["caption"] = json!(
                        locale_chain
                            .iter()
                            .flat_map(|l| [l.as_str(), l.split('-').next().unwrap_or(l)])
                            .find_map(|l| m["caption"][l].as_str())
                            .unwrap_or("")
                    );
                }
                if m["alt"].is_object() {
                    let text = locale_chain
                        .iter()
                        .flat_map(|l| [l.as_str(), l.split('-').next().unwrap_or(l)])
                        .find_map(|l| m["alt"][l].as_str())
                        .map(str::to_owned);
                    if let Some(text) = text {
                        m["view"] = json!(text);
                    }
                }
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
