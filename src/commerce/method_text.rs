//! Shared translated names and descriptions with field-wise shop-main-language inheritance.
use super::*;
pub(crate) use veyndo_core::localized::Text;
pub(crate) use veyndo_core::localized::pick as text_field;
pub(crate) fn validate_text(values: &HashMap<String, Text>, s: &Settings) -> Result<()> {
    veyndo_core::localized::validate_texts(values, &s.locales)
}
pub(crate) fn localized_shipping(v: &Shipping, locale: &str, s: &Settings) -> Value {
    let mut value = json!(v);
    value["name"] =
        json!(text_field(&v.translations, locale, &s.main_locale, false).unwrap_or(&v.name));
    value["description"] =
        json!(text_field(&v.translations, locale, &s.main_locale, true).unwrap_or(""));
    value
}
pub(crate) fn localized_payment(v: &Payment, locale: &str, s: &Settings) -> Value {
    let mut value = json!(v);
    value["name"] =
        json!(text_field(&v.translations, locale, &s.main_locale, false).unwrap_or(&v.name));
    value["description"] =
        json!(text_field(&v.translations, locale, &s.main_locale, true).unwrap_or(""));
    value
}
