//! Shop-owned customer groups; each selects whether its prices are shown gross (consumer) or net (business).
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use veyndo_core::localized::Text;
use veyndo_kernel::verified_kernel;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomerGroup {
    pub id: String,
    pub translations: HashMap<String, Text>,
    pub price_basis: String,
}

/// Whether the named group is priced net; an unknown group is priced like a consumer.
pub fn priced_net(groups: &[CustomerGroup], group: &str) -> bool {
    let entry = groups.iter().find(|g| g.id == group);
    verified_kernel::customer_group_net(
        entry.is_some(),
        entry.is_some_and(|g| g.price_basis == "business"),
    )
}

#[cfg(test)]
mod tests {
    use super::{CustomerGroup, priced_net};

    fn group(id: &str, basis: &str) -> CustomerGroup {
        CustomerGroup {
            id: id.into(),
            translations: Default::default(),
            price_basis: basis.into(),
        }
    }

    #[test]
    fn only_a_known_business_group_is_priced_net() {
        let groups = [group("consumer", "consumer"), group("trade", "business")];
        assert!(priced_net(&groups, "trade"));
        assert!(!priced_net(&groups, "consumer"));
        assert!(!priced_net(&groups, "unknown"));
        assert!(!priced_net(&[], "trade"));
    }
}
