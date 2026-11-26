//! Tenant-owned translated customer groups preserve exact rule IDs and select an explicit existing price/tax presentation basis.
use crate::types::settings::Settings;
use serde_json::json;
use veyndo_app_model::declarations::identifier;
use veyndo_core::localized::validate_texts;
use veyndo_core::{Result, bad};
use veyndo_pricing::types::customer_group::CustomerGroup;

pub fn defaults() -> Vec<CustomerGroup> {
    serde_json::from_value(json!([
      {"id":"consumer","priceBasis":"consumer","translations":{"en-GB":{"name":"Consumers"},"de-DE":{"name":"Privatkunden"},"es-ES":{"name":"Clientes particulares"},"fr-FR":{"name":"Particuliers"}}},
      {"id":"business","priceBasis":"business","translations":{"en-GB":{"name":"Business customers"},"de-DE":{"name":"Geschäftskunden"},"es-ES":{"name":"Clientes profesionales"},"fr-FR":{"name":"Professionnels"}}}
    ])).unwrap()
}
pub fn validate_groups(s: &Settings) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    if s.customer_groups.is_empty() || s.customer_groups.len() > 100 {
        return Err(bad("Invalid customer group count"));
    }
    for g in &s.customer_groups {
        if !identifier(&g.id)
            || g.id.len() > 50
            || !seen.insert(&g.id)
            || !["consumer", "business"].contains(&g.price_basis.as_str())
        {
            return Err(bad("Invalid customer group identity or price basis"));
        }
        validate_texts(&g.translations, &s.locales)?;
        if (g.id == "consumer" && g.price_basis != "consumer")
            || (g.id == "business" && g.price_basis != "business")
        {
            return Err(bad("Built-in customer group price basis is fixed"));
        }
    }
    if !seen.contains(&"consumer".to_string()) || !seen.contains(&"business".to_string()) {
        return Err(bad("Built-in customer groups are required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{defaults, validate_groups};
    use crate::types::settings::Settings;

    fn settings() -> Settings {
        let mut s: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        s.customer_groups = defaults();
        s
    }

    #[test]
    fn the_bundled_groups_are_valid_and_required_with_their_fixed_basis() {
        let s = settings();
        assert_eq!(
            defaults().iter().map(|g| g.id.as_str()).collect::<Vec<_>>(),
            ["consumer", "business"]
        );
        assert!(validate_groups(&s).is_ok());
        let mut swapped = settings();
        swapped.customer_groups[0].price_basis = "business".into();
        assert!(validate_groups(&swapped).is_err());
        let mut missing = settings();
        missing.customer_groups.pop();
        assert!(validate_groups(&missing).is_err());
        let mut none = settings();
        none.customer_groups.clear();
        assert!(validate_groups(&none).is_err());
    }

    #[test]
    fn own_groups_need_a_unique_identifier_and_a_known_basis() {
        let mut s = settings();
        let mut own = s.customer_groups[1].clone();
        own.id = "wholesale".into();
        s.customer_groups.push(own.clone());
        assert!(validate_groups(&s).is_ok());
        s.customer_groups.push(own.clone());
        assert!(validate_groups(&s).is_err());
        s.customer_groups.pop();
        s.customer_groups[2].price_basis = "net".into();
        assert!(validate_groups(&s).is_err());
        s.customer_groups[2].price_basis = "consumer".into();
        s.customer_groups[2].id = "Not an identifier".into();
        assert!(validate_groups(&s).is_err());
    }
}
