//! Bundled MIT country catalogue, tenant-owned overrides and typed region admission.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use veyndo_core::{Result, bad};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Country {
    pub code: String,
    pub alpha3: String,
    pub numeric: String,
    #[serde(default)]
    pub iso_assigned: bool,
    pub continent: String,
    pub name: HashMap<String, String>,
    #[serde(default)]
    pub states: Vec<Subdivision>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Subdivision {
    pub code: String,
    pub name: HashMap<String, String>,
}

/// The bundled countries of the world.
pub fn world() -> &'static [Country] {
    static WORLD: OnceLock<Vec<Country>> = OnceLock::new();
    WORLD.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../fixtures/geography.json"))
            .expect("bundled geography")
    })
}

/// The world with the shop's own definitions replacing or extending it, in code order.
pub fn catalogue(definitions: &[Country]) -> Vec<Country> {
    let mut rows = world().to_vec();
    for item in definitions {
        if let Some(old) = rows.iter_mut().find(|c| c.code == item.code) {
            *old = item.clone();
        } else {
            rows.push(item.clone());
        }
    }
    rows.sort_by(|a, b| a.code.cmp(&b.code));
    rows
}

/// Checks the shop's own definitions and that every enabled country is in its catalogue.
pub fn validate(definitions: &[Country], enabled: &[String]) -> Result<()> {
    let rows = catalogue(definitions);
    let mut seen = HashSet::new();
    if definitions.len() > 300 {
        return Err(bad("Too many country definitions"));
    }
    for c in definitions {
        if c.iso_assigned
            && !world().iter().any(|b| {
                b.iso_assigned && b.code == c.code && b.alpha3 == c.alpha3 && b.numeric == c.numeric
            })
        {
            return Err(bad("Custom countries cannot claim ISO assignment"));
        }
        if !seen.insert(&c.code)
            || c.code.len() != 2
            || !c.code.bytes().all(|x| x.is_ascii_uppercase())
            || (!c.alpha3.is_empty() && c.alpha3.len() != 3)
            || !c.alpha3.bytes().all(|x| x.is_ascii_uppercase())
            || (!c.numeric.is_empty() && c.numeric.len() != 3)
            || !c.numeric.bytes().all(|x| x.is_ascii_digit())
            || !["AF", "AN", "AS", "EU", "NA", "OC", "SA"].contains(&c.continent.as_str())
            || c.name.is_empty()
            || c.name.values().any(|n| n.is_empty() || n.len() > 160)
            || c.states.len() > 500
        {
            return Err(bad("Invalid country definition"));
        }
        let mut states = HashSet::new();
        if c.states.iter().any(|st| {
            !states.insert(&st.code)
                || !st.code.starts_with(&format!("{}-", c.code))
                || st.code.len() > 12
                || st.name.is_empty()
                || st.name.values().any(|n| n.is_empty() || n.len() > 160)
        }) {
            return Err(bad("Invalid country subdivision"));
        }
    }
    if enabled.iter().any(|c| !rows.iter().any(|r| r.code == *c)) {
        return Err(bad("Country missing from catalogue"));
    }
    Ok(())
}

/// All address entry points share tenant-defined country/subdivision validation.
pub fn validate_address(country: &str, region: &str, definitions: &[Country]) -> Result<()> {
    let country = catalogue(definitions)
        .into_iter()
        .find(|c| c.code == country)
        .ok_or(bad("Address country unavailable"))?;
    if !country.states.is_empty() && region.is_empty() {
        return Err(bad("Address region required"));
    }
    if !region.is_empty() && !country.states.iter().any(|s| s.code == region) {
        return Err(bad("Address region is unavailable for this country"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Country, Subdivision, catalogue, validate, validate_address, world};
    use std::collections::HashMap;

    fn custom(code: &str) -> Country {
        Country {
            code: code.into(),
            alpha3: String::new(),
            numeric: String::new(),
            iso_assigned: false,
            continent: "EU".into(),
            name: HashMap::from([("en".to_owned(), "Custom".to_owned())]),
            states: vec![Subdivision {
                code: format!("{code}-N"),
                name: HashMap::from([("en".to_owned(), "North".to_owned())]),
            }],
        }
    }

    #[test]
    fn own_definitions_extend_the_world_without_claiming_iso() {
        let own = [custom("XQ")];
        assert_eq!(catalogue(&own).len(), world().len() + 1);
        assert!(validate(&own, &["XQ".to_owned(), "DE".to_owned()]).is_ok());
        assert!(validate(&own, &["XZ".to_owned()]).is_err());
        let mut claimed = custom("XQ");
        claimed.iso_assigned = true;
        assert!(validate(&[claimed], &[]).is_err());
        let mut foreign_state = custom("XQ");
        foreign_state.states[0].code = "DE-BY".into();
        assert!(validate(&[foreign_state], &[]).is_err());
        assert!(validate(&[custom("XQ"), custom("XQ")], &[]).is_err());
    }

    #[test]
    fn a_country_with_regions_needs_one_of_its_own() {
        let own = [custom("XQ")];
        assert!(validate_address("XQ", "XQ-N", &own).is_ok());
        assert!(validate_address("XQ", "", &own).is_err());
        assert!(validate_address("XQ", "XQ-S", &own).is_err());
        assert!(validate_address("XY", "", &own).is_err());
    }
}
