//! Shared translated names and descriptions with field-wise shop-main-language inheritance.
use crate::types::settings::{Payment, Settings, Shipping};
use serde_json::{Value, json};
use veyndo_core::document::merge;
use veyndo_core::localized::pick as text_field;
/// The serialized method with its name and description in `locale`, each falling back to the main language.
fn localized(
    mut value: Value,
    translations: &std::collections::HashMap<String, veyndo_core::localized::Text>,
    name: &str,
    locale: &str,
    s: &Settings,
) -> Value {
    // A serialized method is an object, so the merge cannot fail.
    let _ = merge(
        &mut value,
        json!({
            "name": text_field(translations, locale, &s.main_locale, false).unwrap_or(name),
            "description": text_field(translations, locale, &s.main_locale, true).unwrap_or(""),
        }),
    );
    value
}
pub fn localized_shipping(v: &Shipping, locale: &str, s: &Settings) -> Value {
    localized(json!(v), &v.translations, &v.name, locale, s)
}
pub fn localized_payment(v: &Payment, locale: &str, s: &Settings) -> Value {
    localized(json!(v), &v.translations, &v.name, locale, s)
}

#[cfg(test)]
mod tests {
    use super::{localized_payment, localized_shipping};
    use crate::types::settings::Settings;
    use serde_json::json;

    #[test]
    fn method_names_fall_back_to_the_main_language_field_by_field() {
        let mut s: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        s.shipping[0].translations = serde_json::from_value(json!({
            "en-GB":{"name":"Pickup","description":"At the counter"},
            "de-DE":{"name":"Abholung","description":null}
        }))
        .unwrap();
        s.payments[0].translations = serde_json::from_value(json!({
            "en-GB":{"name":"Card","description":null},
            "de-DE":{"name":null,"description":"Sofort"}
        }))
        .unwrap();
        let shipping = localized_shipping(&s.shipping[0], "de-DE", &s);
        assert_eq!(shipping["id"], "pickup");
        assert_eq!(shipping["name"], "Abholung");
        assert_eq!(shipping["description"], "At the counter");
        let payment = localized_payment(&s.payments[0], "de-DE", &s);
        assert_eq!(payment["name"], "Card");
        assert_eq!(payment["description"], "Sofort");
        let unknown = localized_payment(&s.payments[0], "it-IT", &s);
        assert_eq!(unknown["name"], "Card");
        assert_eq!(unknown["description"], "");
        s.payments[0].translations.clear();
        assert_eq!(
            localized_payment(&s.payments[0], "de-DE", &s)["name"],
            "card"
        );
    }
}
