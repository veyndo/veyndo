//! The product edit payload: translated texts, multilingual metadata and the optional commerce and identity fields.
use crate::domain::rich::validate_rich;
use crate::types::product_fields::ProductFields;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use veyndo_core::localized::Languages;
use veyndo_core::{Result, bad};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edit {
    #[serde(default, rename = "id")]
    pub _id: Option<String>,
    #[serde(default, rename = "channels")]
    pub _channels: Value,
    #[serde(default, rename = "mainLocale")]
    pub _main_locale: Value,
    #[serde(default, rename = "availableLocales")]
    pub _available_locales: Value,
    /// The aggregate clock; it also moves when stock changes.
    pub revision: i64,
    #[serde(default)]
    pub settings_revision: Option<i64>,
    /// The content clock; an edit that sends it and no stock is checked against it instead.
    #[serde(default)]
    pub content_revision: Option<i64>,
    pub translations: HashMap<String, Translation>,
    pub extra: Extra,
    #[serde(default)]
    pub commerce: Option<ProductFields>,
    #[serde(default)]
    pub catalog: Option<CatalogFields>,
}

/// Name and description in one language; `None` inherits the main language.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Translation {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Extra {
    #[serde(default)]
    pub currency_prices: Value,
    #[serde(default)]
    pub price_currency: Option<String>,
    #[serde(default)]
    pub demo: Value,
    #[serde(default)]
    pub automation: Value,
    #[serde(default)]
    pub tax_class_id: Option<String>,
    #[serde(default)]
    pub seo: HashMap<String, Seo>,
    #[serde(default)]
    pub specifications: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub cross_selling: Vec<String>,
    #[serde(default)]
    pub shipping_free: bool,
    #[serde(default)]
    pub digital: bool,
    #[serde(default)]
    pub rich_description: Value,
    #[serde(default)]
    pub identity: Value,
    #[serde(default)]
    pub compliance: Value,
    /// Names of variant option values per content language; see `domain::option_values`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub option_values: crate::domain::option_values::OptionValues,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Seo {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
}

/// Identity and associations of a product: number, categories, channels and the variant link.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogFields {
    pub active: bool,
    pub product_number: String,
    pub category_ids: Vec<String>,
    #[serde(default)]
    pub sales_channel_ids: Option<Vec<String>>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub options: HashMap<String, String>,
}

impl CatalogFields {
    pub fn validate(&self) -> Result<()> {
        if self.product_number.trim().is_empty()
            || self.product_number.len() > 100
            || self.category_ids.len() > 100
            || self.options.len() > 40
            || self.options.iter().any(|(k, v)| {
                k.trim().is_empty() || k.len() > 100 || v.trim().is_empty() || v.len() > 200
            })
        {
            return Err(bad("Invalid product identity or options"));
        }
        Ok(())
    }
}

/// Which clock an edit is checked against, and the field a mismatch is reported at.
pub struct Expected {
    pub current: i64,
    pub expected: i64,
    pub pointer: &'static str,
}

impl Edit {
    /// Bounds on the translated texts; only enabled languages may be written.
    pub fn validate_translations(&self, languages: Languages) -> Result<()> {
        if self.translations.len() > 100 || self.translations.is_empty() {
            return Err(bad("Product translations exceed limits"));
        }
        for (lang, tr) in &self.translations {
            if !languages.allowed(lang)
                || tr
                    .name
                    .as_ref()
                    .is_some_and(|v| v.is_empty() || v.len() > 200)
                || tr.description.as_ref().is_some_and(|v| v.len() > 4000)
            {
                return Err(bad("Invalid translated product text"));
            }
        }
        Ok(())
    }

    /// Name and description in the main language; the name is required, a missing description is empty.
    pub fn main_text(&self, languages: Languages) -> Result<(String, String)> {
        let main = self
            .translations
            .get(&languages.key(languages.main))
            .ok_or(bad("Main product language required"))?;
        Ok((
            main.name.clone().ok_or(bad("Main product name required"))?,
            main.description.clone().unwrap_or_default(),
        ))
    }

    /// Limits, the tax class, SEO, specifications and the rich description.
    pub fn validate_metadata(
        &self,
        languages: Languages,
        tax_class_known: impl Fn(&str) -> bool,
    ) -> Result<()> {
        let extra = &self.extra;
        if extra.cross_selling.len() > 20
            || extra.seo.len() > 100
            || extra.specifications.len() > 100
        {
            return Err(bad("Product metadata exceeds limits"));
        }
        if extra
            .tax_class_id
            .as_ref()
            .is_some_and(|id| !tax_class_known(id))
        {
            return Err(bad("Unknown product tax class"));
        }
        for (lang, seo) in &extra.seo {
            if !languages.allowed(lang)
                || seo.title.as_ref().is_some_and(|v| v.len() > 200)
                || seo.description.as_ref().is_some_and(|v| v.len() > 500)
                || seo
                    .slug
                    .as_ref()
                    .is_some_and(|v| v.len() > 200 || v.contains(['?', '#', '/']))
            {
                return Err(bad("Invalid SEO metadata"));
            }
        }
        for (lang, specs) in &extra.specifications {
            if !languages.allowed(lang)
                || specs.len() > 40
                || specs
                    .iter()
                    .any(|(k, v)| k.is_empty() || k.len() > 100 || v.len() > 1000)
            {
                return Err(bad("Invalid specification metadata"));
            }
        }
        crate::domain::option_values::validate(&extra.option_values, languages)?;
        if !extra.rich_description.is_null() {
            validate_rich(&extra.rich_description)?;
            if extra
                .rich_description
                .as_object()
                .is_some_and(|map| map.keys().any(|lang| !languages.allowed(lang)))
            {
                return Err(bad("Rich description language is not enabled"));
            }
        }
        Ok(())
    }

    /// The commerce fields with their translated image texts, the identity fields and the identity metadata.
    pub fn validate_fields(&self, languages: Languages) -> Result<()> {
        if let Some(fields) = &self.commerce {
            fields.validate()?;
            for media in fields.media.as_array().into_iter().flatten() {
                for (field, limit) in [("alt", 400), ("caption", 600)] {
                    if let Some(value) = media.get(field) {
                        let translations = value
                            .as_object()
                            .ok_or(bad("Image text must be translations"))?;
                        if translations.len() > 100
                            || translations.iter().any(|(l, v)| {
                                !languages.allowed(l)
                                    || !(v.is_null()
                                        || v.as_str().is_some_and(|s| s.len() <= limit))
                            })
                        {
                            return Err(bad("Invalid image text translation"));
                        }
                    }
                }
            }
        }
        if let Some(c) = &self.catalog {
            c.validate()?;
        }
        if !self.extra.identity.is_null()
            && (!self.extra.identity.is_object() || self.extra.identity.to_string().len() > 8000)
        {
            return Err(bad("Invalid product identity metadata"));
        }
        Ok(())
    }

    /// An edit that sends the content clock and no stock is checked against the content clock,
    /// so a sale between load and save does not reject it. Every other edit keeps the aggregate clock.
    pub fn expected(&self, revision: i64, content_revision: i64) -> Expected {
        let content_only = self.content_revision.is_some()
            && self
                .commerce
                .as_ref()
                .is_none_or(|fields| fields.stock.is_none());
        match self.content_revision {
            Some(expected) if content_only => Expected {
                current: content_revision,
                expected,
                pointer: "/contentRevision",
            },
            _ => Expected {
                current: revision,
                expected: self.revision,
                pointer: "/revision",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Edit;
    use serde_json::{Value, json};
    use veyndo_core::localized::Languages;

    fn edit(change: impl FnOnce(&mut Value)) -> Edit {
        let mut value = json!({
            "revision":7,
            "translations":{"es":{"name":"Lámpara","description":"Luz"},"de":{"name":null}},
            "extra":{}
        });
        change(&mut value);
        serde_json::from_value(value).unwrap()
    }
    fn commerce(stock: Option<i32>) -> Value {
        let mut fields = json!({
            "price":10.0,"taxRate":19.0,"minPurchase":1,"purchaseSteps":1,"maxPurchase":null,
            "deliveryDays":2,"listPrice":null,"advancedPrices":[],"media":[],"properties":{}
        });
        if let Some(stock) = stock {
            fields["stock"] = json!(stock);
        }
        fields
    }

    #[test]
    fn the_main_language_is_required_and_other_languages_inherit() {
        let enabled = vec!["es-ES".to_owned(), "de-DE".to_owned()];
        let languages = Languages {
            main: "es-ES",
            enabled: &enabled,
        };
        let ok = edit(|_| {});
        assert!(ok.validate_translations(languages).is_ok());
        assert_eq!(
            ok.main_text(languages).unwrap(),
            ("Lámpara".into(), "Luz".into())
        );
        let foreign = edit(|v| v["translations"]["fr"] = json!({"name":"Lampe"}));
        assert!(foreign.validate_translations(languages).is_err());
        let no_main = edit(|v| v["translations"] = json!({"de":{"name":"Lampe"}}));
        assert!(no_main.main_text(languages).is_err());
        let inherits = edit(|v| v["translations"]["es"] = json!({"name":null}));
        assert!(inherits.main_text(languages).is_err());
    }

    #[test]
    fn metadata_is_bounded_and_uses_enabled_languages_and_known_tax_classes() {
        let enabled = vec!["es-ES".to_owned()];
        let languages = Languages {
            main: "es-ES",
            enabled: &enabled,
        };
        let known = |id: &str| id == "standard";
        assert!(
            edit(|v| v["extra"] = json!({"taxClassId":"standard","seo":{"es":{"slug":"lampara"}}}))
                .validate_metadata(languages, known)
                .is_ok()
        );
        for extra in [
            json!({"taxClassId":"unknown"}),
            json!({"seo":{"de":{"title":"Lampe"}}}),
            json!({"seo":{"es":{"slug":"a/b"}}}),
            json!({"specifications":{"es":{"":"x"}}}),
            json!({"crossSelling":(0..21).map(|n| n.to_string()).collect::<Vec<_>>()}),
            json!({"richDescription":{"de":[]}}),
        ] {
            assert!(
                edit(|v| v["extra"] = extra.clone())
                    .validate_metadata(languages, known)
                    .is_err(),
                "{extra}"
            );
        }
    }

    #[test]
    fn image_texts_are_translations_in_enabled_languages() {
        let enabled = vec!["es-ES".to_owned()];
        let languages = Languages {
            main: "es-ES",
            enabled: &enabled,
        };
        let with_media = |media: Value| {
            edit(|v| {
                v["commerce"] = commerce(None);
                v["commerce"]["media"] = media;
            })
        };
        let item = |alt: Value| json!([{"url":"/media/a.jpg","id":"a","view":"front","alt":alt}]);
        assert!(
            with_media(item(json!({"es":"Lámpara","es-ES":null})))
                .validate_fields(languages)
                .is_ok()
        );
        assert!(
            with_media(item(json!("Lámpara")))
                .validate_fields(languages)
                .is_err()
        );
        assert!(
            with_media(item(json!({"de":"Lampe"})))
                .validate_fields(languages)
                .is_err()
        );
        assert!(
            edit(|v| v["extra"]["identity"] = json!("x"))
                .validate_fields(languages)
                .is_err()
        );
    }

    #[test]
    fn a_content_edit_without_stock_is_checked_against_the_content_clock() {
        let content = edit(|v| {
            v["contentRevision"] = json!(3);
            v["commerce"] = commerce(None);
        });
        let check = content.expected(9, 3);
        assert_eq!(
            (check.current, check.expected, check.pointer),
            (3, 3, "/contentRevision")
        );
        let with_stock = edit(|v| {
            v["contentRevision"] = json!(3);
            v["commerce"] = commerce(Some(5));
        });
        let check = with_stock.expected(9, 3);
        assert_eq!(
            (check.current, check.expected, check.pointer),
            (9, 7, "/revision")
        );
        let legacy = edit(|_| {});
        assert_eq!(legacy.expected(9, 3).pointer, "/revision");
    }
}
