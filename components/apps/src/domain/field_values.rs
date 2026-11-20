//! Native currency/content/asset policy for the shared pure field validator.
use crate::domain::reference_policy::ReferencePolicy;
use crate::domain::{model_contract, relations};
use serde_json::Value;
use veyndo_app_model::{Entity, Field};
use veyndo_core::Result;
use veyndo_core::localized::valid_locale_key;
use veyndo_kernel::verified_kernel;
use veyndo_pricing::types::currency as currencies;

/// The native value rules every app field, configuration and view is checked with.
pub struct Policy;
impl veyndo_app_model::fields::ValuePolicy for Policy {
    fn locale(&self, key: &str) -> bool {
        valid_locale_key(key)
    }
    fn money(&self, value: &Value) -> bool {
        money(value)
    }
    fn richtext(&self, value: &Value) -> bool {
        veyndo_catalog::domain::rich::validate_rich(value).is_ok()
    }
    fn relations(&self, value: &Value) -> bool {
        relations::valid(value)
    }
}
impl veyndo_app_model::declarations::ReferencePolicy for Policy {
    fn admitted(&self, product: bool, private: bool) -> bool {
        veyndo_app_model::declarations::ReferencePolicy::admitted(
            &ReferencePolicy,
            product,
            private,
        )
    }
}
impl veyndo_app_model::configuration::Policy for Policy {
    fn known_currency(&self, code: &str) -> bool {
        currencies::scale(code).is_some()
    }
}
impl veyndo_app_model::views::Policy for Policy {
    fn public_read(&self, writes: bool) -> bool {
        verified_kernel::app_read_admissible(true, writes)
    }
}
/// Checks the fields of a record against its entity.
pub fn validate(e: &Entity, value: &Value) -> Result<()> {
    veyndo_app_model::fields::validate(e, value, &Policy).map_err(model_contract::error)
}
/// Whether one value fits its field.
pub fn value(f: &Field, value: &Value) -> bool {
    veyndo_app_model::fields::value(f, value, &Policy)
}
fn money(v: &Value) -> bool {
    let Ok(m) = serde_json::from_value::<veyndo_pricing::money::Money>(v.clone()) else {
        return false;
    };
    currencies::scale(m.currency().code()) == Some(m.currency().scale())
        && v.as_object().is_some_and(|o| o.len() == 2)
        && v["currency"].as_object().is_some_and(|o| o.len() == 2)
        // JSON clients must not lose precision. Larger amounts require a future string minor-unit wire revision.
        && m.minor().unsigned_abs() <= 9_007_199_254_740_991
}
#[cfg(test)]
mod tests {
    use super::value;
    use serde_json::json;
    use veyndo_app_model::Field;
    fn field(kind: &str) -> Field {
        serde_json::from_value(json!({"name":"value","kind":kind})).unwrap()
    }
    #[test]
    fn exact_typed_values_reject_coercion_invalid_dates_and_currency_scale() {
        for (kind, v) in [
            ("date", json!("2026-02-29")),
            ("datetime", json!("2026-10-08")),
            ("decimal", json!(0.1)),
            ("decimal", json!("1e3")),
            (
                "money",
                json!({"minor":123,"currency":{"code":"JPY","scale":2}}),
            ),
            (
                "richtext",
                json!({"en":[{"type":"image","url":"javascript:alert(1)"}]}),
            ),
        ] {
            assert!(!value(&field(kind), &v));
        }
        for (kind, v) in [
            ("date", json!("2024-02-29")),
            ("datetime", json!("2026-10-08T12:00:00Z")),
            ("decimal", json!("123456789012345678.12345678")),
            (
                "money",
                json!({"minor":123,"currency":{"code":"JPY","scale":0}}),
            ),
        ] {
            assert!(value(&field(kind), &v));
        }
    }
    #[test]
    fn the_policy_admits_shop_locales_distinct_relations_and_public_reads_only() {
        use super::Policy;
        use veyndo_app_model::configuration::Policy as _;
        use veyndo_app_model::declarations::ReferencePolicy as _;
        use veyndo_app_model::fields::ValuePolicy as _;
        use veyndo_app_model::views::Policy as _;
        assert!(Policy.locale("de-DE"));
        assert!(!Policy.locale("not a locale"));
        assert!(Policy.relations(&json!(["a", "b"])));
        assert!(!Policy.relations(&json!(["a", "a"])));
        assert!(Policy.known_currency("EUR"));
        assert!(!Policy.known_currency("ZZZ"));
        assert!(Policy.public_read(false));
        assert!(!Policy.public_read(true));
        assert_eq!(
            Policy.admitted(true, false),
            veyndo_kernel::verified_kernel::app_core_reference_admissible(true, false)
        );
    }

    #[test]
    fn a_record_is_checked_against_its_entity() {
        use super::validate;
        use crate::domain::approval::built_in;
        use veyndo_app_model::manifest::Manifest;
        let m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        assert!(validate(&m.entities[0], &json!("not an object")).is_err());
        assert!(validate(&m.entities[0], &json!({"no_such_field":1})).is_err());
    }
}
