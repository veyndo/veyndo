//! Bounds every set of cart lines keeps: at most 100 distinct products, each once, in a sane quantity.
use std::collections::HashSet;
use veyndo_core::{Result, bad};
use veyndo_pricing::domain::cart::Item;

pub fn validate(items: &[Item]) -> Result<()> {
    if items.len() > 100 {
        return Err(bad("Maximum 100 distinct items"));
    }
    let mut seen = HashSet::new();
    for i in items {
        if i.quantity == 0 || i.quantity > 10000 || !seen.insert(&i.id) {
            return Err(bad(
                "Items must have unique IDs and quantities from 1 to 10000",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use veyndo_pricing::domain::cart::Item;

    fn item(id: &str, quantity: u32) -> Item {
        Item {
            id: id.into(),
            quantity,
        }
    }

    #[test]
    fn lines_are_unique_bounded_and_at_most_a_hundred() {
        assert!(validate(&[]).is_ok());
        assert!(validate(&[item("a", 1), item("b", 10000)]).is_ok());
        assert!(validate(&[item("a", 0)]).is_err());
        assert!(validate(&[item("a", 10001)]).is_err());
        assert!(validate(&[item("a", 1), item("a", 2)]).is_err());
        let many = |n: usize| {
            (0..n)
                .map(|i| item(&format!("p{i}"), 1))
                .collect::<Vec<_>>()
        };
        assert!(validate(&many(100)).is_ok());
        assert!(validate(&many(101)).is_err());
    }
}
