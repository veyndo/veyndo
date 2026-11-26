//! What a change of the currency configuration must not do: relabel stored amounts or remove a currency still in use.
use crate::ports::References;
use crate::store::currency_usage;
use crate::types::currency::Config;
use sqlx::PgConnection;
use veyndo_core::{Result, conflict};

/// The currencies that `next` no longer defines.
pub fn removed(old: &Config, next: &Config) -> Vec<String> {
    old.definitions
        .iter()
        .filter(|d| !next.definitions.iter().any(|n| n.code == d.code))
        .map(|d| d.code.clone())
        .collect()
}

pub async fn guard(
    references: &dyn References,
    tx: &mut PgConnection,
    tenant: &str,
    old: &Config,
    next: &Config,
) -> Result<()> {
    if old.pricing_currency != next.pricing_currency {
        return Err(conflict(
            "The original monetary storage currency cannot be relabelled",
        ));
    }
    if currency_usage::any_in_use(references, tx, tenant, &removed(old, next)).await? {
        return Err(conflict(
            "Currency is referenced by product prices, open carts or purchasing units; remove channel availability instead",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::removed;
    use crate::types::currency::{Config, Definition};

    #[test]
    fn only_definitions_missing_from_the_new_configuration_count_as_removed() {
        let mut old = Config::default();
        old.definitions.push(Definition {
            code: "USD".into(),
            scale: 2,
            rate: "1.25".into(),
            strategy: "automatic".into(),
        });
        let mut next = old.clone();
        assert!(removed(&old, &next).is_empty());
        // Changing a rate or disabling a currency for a channel removes nothing.
        next.definitions[1].rate = "1.30".into();
        next.enabled.clear();
        assert!(removed(&old, &next).is_empty());
        next.definitions.pop();
        assert_eq!(removed(&old, &next), ["USD"]);
        assert!(removed(&next, &old).is_empty());
    }
}
