//! The budget of app configurations one cart may carry.
use serde_json::json;
use std::collections::HashMap;
use veyndo_core::{Result, bad};
use veyndo_shop::types::cart::AppConfiguration;

/// Limits a cart to 100 configurations and 64 KiB of configuration data.
pub fn configuration_budget(configs: &HashMap<String, AppConfiguration>) -> Result<()> {
    if configs.len() > 100 || json!(configs).to_string().len() > 65536 {
        return Err(bad(
            "Cart configuration exceeds its 100-selection or 64 KiB budget",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::configuration_budget;
    use serde_json::json;
    use std::collections::HashMap;
    use veyndo_shop::types::cart::AppConfiguration;

    fn configuration(text: &str) -> AppConfiguration {
        serde_json::from_value(json!({"app":"sample","productId":"p","fields":{"note":text},"feeMinor":0,"ruleRevision":1,"appVersion":"1.0.0","entity":"choices","record":"one"}))
            .unwrap()
    }

    #[test]
    fn a_cart_carries_at_most_100_configurations_and_64_kib() {
        let small = |n: usize| {
            (0..n)
                .map(|i| (format!("line-{i}"), configuration("x")))
                .collect::<HashMap<_, _>>()
        };
        assert!(configuration_budget(&small(100)).is_ok());
        assert!(configuration_budget(&small(101)).is_err());
        let large = HashMap::from([("line".to_owned(), configuration(&"x".repeat(70_000)))]);
        assert!(configuration_budget(&large).is_err());
    }
}
