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
