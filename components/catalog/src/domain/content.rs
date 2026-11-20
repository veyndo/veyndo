//! Field-level content fallback: a missing or `null` translation inherits the main language, an empty one stays empty.
use serde_json::{Value, json};

/// The text for `locale`, falling back to its base language and then to the main language; empty when none is set.
pub fn translated_string<'a>(values: &'a Value, locale: &str, main: &str) -> &'a str {
    [
        locale,
        locale.split('-').next().unwrap_or(locale),
        main,
        main.split('-').next().unwrap_or(main),
    ]
    .into_iter()
    .find_map(|key| values[key].as_str())
    .unwrap_or("")
}
/// Field-wise inheritance: each field comes from the selected language unless it is missing or `null` there.
pub fn translated_object(values: &Value, locale: &str, main: &str) -> Value {
    let mut result = json!({});
    for key in [
        main.split('-').next().unwrap(),
        main,
        locale.split('-').next().unwrap(),
        locale,
    ] {
        if let Some(object) = values[key].as_object() {
            for (field, value) in object {
                if !value.is_null() {
                    result[field] = value.clone();
                }
            }
        }
    }
    result
}
/// Writes the effective SEO, specification and rich description of the first language in `chain` into `extra`.
pub fn localize_extra(extra: &mut Value, chain: &[String]) {
    let Some(selected) = chain.first() else {
        return;
    };
    for field in ["seo", "specifications", "richDescription"] {
        let mut effective = Value::Null;
        for locale in chain.iter().rev() {
            let base = locale.split('-').next().unwrap();
            for key in [base, locale.as_str()] {
                let value = &extra[field][key];
                if value.is_null() {
                    continue;
                }
                if value.is_object() && effective.is_object() {
                    for (k, v) in value.as_object().unwrap() {
                        if !v.is_null() {
                            effective[k] = v.clone();
                        }
                    }
                } else {
                    effective = value.clone();
                }
            }
        }
        if !effective.is_null() {
            extra[field][selected.split('-').next().unwrap()] = effective.clone();
            extra[field][selected] = effective;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{localize_extra, translated_object, translated_string};
    use serde_json::json;
    #[test]
    fn each_field_inherits_main_and_preserves_blank() {
        let map = json!({"es":{"name":"Nombre","description":"Original","slug":"producto"},"de":{"name":null,"description":""}});
        let out = translated_object(&map, "de-DE", "es-ES");
        assert_eq!(out["name"], "Nombre");
        assert_eq!(out["description"], "");
        assert_eq!(out["slug"], "producto");
    }
    #[test]
    fn metadata_uses_resolved_language_chain() {
        let mut extra = json!({"seo":{"en":{"title":"EN"},"es":{"title":"ES","description":"Original"},"de":{"title":null}},"richDescription":{"es":[{"type":"paragraph","text":"Hola"}]}});
        localize_extra(&mut extra, &["de-DE".into(), "es-ES".into()]);
        assert_eq!(extra["seo"]["de"]["title"], "ES");
        assert_eq!(extra["richDescription"]["de"][0]["text"], "Hola");
    }
    #[test]
    fn a_text_falls_back_to_its_base_language_then_to_the_main_language() {
        let map = json!({"de-AT":"Sackerl","de":"Tüte","es-ES":"Bolsa","fr":null});
        assert_eq!(translated_string(&map, "de-AT", "es-ES"), "Sackerl");
        assert_eq!(translated_string(&map, "de-DE", "es-ES"), "Tüte");
        assert_eq!(translated_string(&map, "fr-FR", "es-ES"), "Bolsa");
        assert_eq!(
            translated_string(&json!({"es":"Bolsa"}), "it-IT", "es-ES"),
            "Bolsa"
        );
        assert_eq!(
            translated_string(&json!({"en":"Bag"}), "it-IT", "es-ES"),
            ""
        );
        assert_eq!(translated_string(&json!({"de":""}), "de-DE", "es-ES"), "");
        assert_eq!(translated_string(&json!(null), "de-DE", "es-ES"), "");
    }
}
