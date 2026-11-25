//! Configuration mutations publish minimized identifiers through the native outbox/Flows; no cart capability or shopper text is disclosed.
use serde_json::json;
use std::collections::HashMap;
use veyndo_core::Result;
use veyndo_shop::types::cart::{AppConfiguration, StoredCart};

/// Queues one minimized change event per app whose configurations in the cart changed.
pub async fn configuration_events(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    before: &HashMap<String, AppConfiguration>,
) -> Result<()> {
    let mut apps = std::collections::BTreeSet::new();
    for config in before.values().chain(c.data.app_configurations.values()) {
        apps.insert(config.app.as_str());
    }
    for app in apps {
        let old = before
            .iter()
            .filter(|(_, v)| v.app == app)
            .collect::<std::collections::BTreeMap<_, _>>();
        let new = c
            .data
            .app_configurations
            .iter()
            .filter(|(_, v)| v.app == app)
            .collect::<std::collections::BTreeMap<_, _>>();
        if json!(old) == json!(new) {
            continue;
        }
        let products = new
            .values()
            .map(|v| &v.product_id)
            .collect::<std::collections::BTreeSet<_>>();
        veyndo_core::store::outbox::publish_on(&mut **tx, &c.tenant, "app.configuration.changed", &json!({"app":app,"cartId":c.id,"revision":c.revision,"salesChannelId":c.data.sales_channel,"operation":if new.is_empty() {"removed"} else {"configured"},"products":products})).await?;
    }
    Ok(())
}
