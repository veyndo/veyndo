//! Priority-based destination rules; current tax law is merchant configuration, not bundled tax advice.
use crate::domain::validation::Conditions;
use crate::types::settings::{Settings, TaxConfig};
use veyndo_app_model::declarations::identifier;
use veyndo_core::{Result, bad};
use veyndo_shipping::domain::geography::catalogue;

/// Bounds of one tax class's destination rules; `conditions` checks an attached rule condition.
pub fn validate(t: &TaxConfig, s: &Settings, conditions: Conditions<'_>) -> Result<()> {
    let world = catalogue(&s.country_definitions);
    let mut ids = std::collections::HashSet::new();
    if t.rules.len() > 500
        || t.default_rate
            .is_some_and(|x| !x.is_finite() || !(0. ..=100.).contains(&x))
    {
        return Err(bad("Invalid default tax rate or rule limit"));
    }
    for r in &t.rules {
        let country = world
            .iter()
            .find(|c| c.code == r.country)
            .ok_or(bad("Unknown tax rule country"))?;
        if !ids.insert(&r.id)
            || !identifier(&r.id)
            || !r.rate.is_finite()
            || !(0. ..=100.).contains(&r.rate)
            || r.states
                .iter()
                .any(|st| !country.states.iter().any(|x| x.code == *st))
            || r.postal_codes.len() > 100
            || r.postal_prefixes.len() > 100
            || r.postal_codes
                .iter()
                .chain(r.postal_prefixes.iter())
                .any(|v| v.is_empty() || v.len() > 32)
        {
            return Err(bad("Invalid destination tax rule"));
        }
        match (&r.postal_from, &r.postal_to) {
            (Some(a), Some(b))
                if a.len() == b.len()
                    && a <= b
                    && a.bytes().all(|x| x.is_ascii_digit())
                    && b.bytes().all(|x| x.is_ascii_digit()) => {}
            (None, None) => {}
            _ => return Err(bad("Postal ranges require equal-length numeric bounds")),
        }
        for d in [&r.active_from, &r.active_until].into_iter().flatten() {
            if d.len() != 10 {
                return Err(bad("Tax dates require YYYY-MM-DD"));
            }
            chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|_| bad("Tax dates require YYYY-MM-DD"))?;
        }
        if let (Some(a), Some(b)) = (&r.active_from, &r.active_until)
            && a > b
        {
            return Err(bad("Tax validity range is inverted"));
        }
        if let Some(c) = &r.condition {
            conditions(c)?;
        }
    }
    Ok(())
}

/// The destination a checkout selection names, resolved against the priority rules of one tax class.
pub fn destination_rate(
    t: &TaxConfig,
    c: &crate::types::settings::CheckoutSelection,
    today: &str,
) -> Result<f64> {
    let address = c.address.as_ref();
    veyndo_pricing::service::tax::destination_rate(
        t,
        &veyndo_pricing::types::tax::Destination {
            country: &c.country,
            state: address.map(|a| a.country_state_id.as_str()).unwrap_or(""),
            postal_code: address.map(|a| a.postal_code.as_str()).unwrap_or(""),
        },
        today,
    )
}

#[cfg(test)]
mod tests {
    use super::{destination_rate, validate};
    use crate::types::settings::{CheckoutSelection, Settings};
    use serde_json::{Value, json};
    use veyndo_core::bad;

    fn settings() -> Settings {
        serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap()
    }

    fn rule() -> Value {
        json!({"id":"berlin","country":"DE","rate":7.0,"postalPrefixes":["10"]})
    }

    fn check(rule: Value) -> bool {
        let s = settings();
        let mut t = s.taxes[0].clone();
        t.rules = vec![serde_json::from_value(rule).unwrap()];
        validate(&t, &s, &|_| Ok(())).is_ok()
    }

    #[test]
    fn destination_rules_are_bounded_and_name_known_places() {
        assert!(check(rule()));
        for (key, value) in [
            ("country", json!("ZZ")),
            ("rate", json!(101.0)),
            ("rate", json!(-1.0)),
            ("id", json!("Not valid")),
            ("states", json!(["DE-XX"])),
            ("postalCodes", json!([""])),
            ("postalFrom", json!("10000")),
            ("activeFrom", json!("2026-13-01")),
            ("activeFrom", json!("1.1.2026")),
        ] {
            let mut changed = rule();
            changed[key] = value;
            assert!(!check(changed), "{key}");
        }
        let mut range = rule();
        range["postalFrom"] = json!("10000");
        range["postalTo"] = json!("19999");
        assert!(check(range.clone()));
        range["postalTo"] = json!("09999");
        assert!(!check(range.clone()));
        range["postalTo"] = json!("199");
        assert!(!check(range));
        let mut dates = rule();
        dates["activeFrom"] = json!("2026-02-01");
        dates["activeUntil"] = json!("2026-01-01");
        assert!(!check(dates));
    }

    #[test]
    fn rule_counts_default_rates_duplicates_and_conditions_are_checked() {
        let s = settings();
        let mut t = s.taxes[0].clone();
        t.default_rate = Some(120.0);
        assert!(validate(&t, &s, &|_| Ok(())).is_err());
        t.default_rate = Some(5.0);
        assert!(validate(&t, &s, &|_| Ok(())).is_ok());
        t.rules = vec![
            serde_json::from_value(rule()).unwrap(),
            serde_json::from_value(rule()).unwrap(),
        ];
        assert!(validate(&t, &s, &|_| Ok(())).is_err());
        let mut conditional = rule();
        conditional["condition"] = json!({"field":"x"});
        t.rules = vec![serde_json::from_value(conditional).unwrap()];
        assert!(validate(&t, &s, &|_| Ok(())).is_ok());
        assert!(validate(&t, &s, &|_| Err(bad("refused"))).is_err());
    }

    #[test]
    fn the_selection_address_picks_the_destination_rule() {
        let s = settings();
        let mut t = s.taxes[0].clone();
        t.rules = vec![serde_json::from_value(rule()).unwrap()];
        let mut c = CheckoutSelection::defaults();
        c.country = "DE".into();
        assert_eq!(destination_rate(&t, &c, "2026-01-01").ok(), Some(19.0));
        c.address = Some(
            serde_json::from_value(
                json!({"street":"Weg 1","postalCode":"10115","city":"Berlin","country":"DE"}),
            )
            .unwrap(),
        );
        assert_eq!(destination_rate(&t, &c, "2026-01-01").ok(), Some(7.0));
        c.country = "IT".into();
        assert!(destination_rate(&t, &c, "2026-01-01").is_err());
    }
}
