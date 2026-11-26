//! The parts a composed record view may show: always the records, at most three, and only what fits the entity.
use veyndo_core::{Result, bad};

pub fn validate(entity: &str, components: &[String]) -> Result<()> {
    if !["products", "customers", "orders"].contains(&entity)
        || components.is_empty()
        || components.len() > 3
        || !components.iter().any(|c| c == "records")
        || components
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != components.len()
        || components.iter().any(|c| {
            !matches!(
                c.as_str(),
                "records" | "details" | "intake" | "shippingPolicy"
            ) || (c == "intake" && entity != "products")
                || (c == "shippingPolicy" && entity != "orders")
        })
    {
        return Err(bad("Invalid agent workspace components"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;

    fn check(entity: &str, components: &[&str]) -> bool {
        validate(
            entity,
            &components
                .iter()
                .map(|c| (*c).to_owned())
                .collect::<Vec<_>>(),
        )
        .is_ok()
    }

    #[test]
    fn a_view_always_shows_records_and_only_parts_that_fit_its_entity() {
        assert!(check("products", &["records"]));
        assert!(check("products", &["records", "details", "intake"]));
        assert!(check("orders", &["shippingPolicy", "records"]));
        assert!(check("customers", &["records", "details"]));
        assert!(!check("payments", &["records"]));
        assert!(!check("products", &[]));
        assert!(!check("products", &["details"]));
        assert!(!check("products", &["records", "records"]));
        assert!(!check("products", &["records", "chart"]));
        assert!(!check("orders", &["records", "intake"]));
        assert!(!check("products", &["records", "shippingPolicy"]));
        assert!(!check(
            "orders",
            &["records", "details", "shippingPolicy", "intake"]
        ));
    }
}
