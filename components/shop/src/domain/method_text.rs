//! Shared translated names and descriptions with field-wise shop-main-language inheritance.
use crate::types::settings::{Payment, Settings, Shipping};
use serde_json::{Value, json};
use veyndo_core::localized::pick as text_field;
pub fn localized_shipping(v: &Shipping, locale: &str, s: &Settings) -> Value {
    let mut value = json!(v);
    value["name"] =
        json!(text_field(&v.translations, locale, &s.main_locale, false).unwrap_or(&v.name));
    value["description"] =
        json!(text_field(&v.translations, locale, &s.main_locale, true).unwrap_or(""));
    value
}
pub fn localized_payment(v: &Payment, locale: &str, s: &Settings) -> Value {
    let mut value = json!(v);
    value["name"] =
        json!(text_field(&v.translations, locale, &s.main_locale, false).unwrap_or(&v.name));
    value["description"] =
        json!(text_field(&v.translations, locale, &s.main_locale, true).unwrap_or(""));
    value
}
