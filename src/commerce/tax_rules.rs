//! Priority-based destination rules; current tax law is merchant configuration, not bundled tax advice.
use super::*;
pub(crate) fn validate_tax_rules(t: &TaxConfig, s: &Settings) -> Result<()> {
    let world = super::geography::catalogue(s);
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
            || !apps::identifier(&r.id)
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
            marketing::validate_condition(c)?;
        }
    }
    Ok(())
}
pub(crate) fn destination_rate(t: &TaxConfig, c: &CheckoutSelection, today: &str) -> Result<f64> {
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
