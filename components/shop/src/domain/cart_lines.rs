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
