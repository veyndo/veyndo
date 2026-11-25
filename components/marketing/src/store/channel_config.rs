//! Shared transactional channel validation for merchant edits, history restoration and selective release. Category and product existence come from the catalog store on the same transaction.
use crate::types::channel::{Channel, validate_pos};
use serde::Deserialize;
use serde_json::Value;
use veyndo_catalog::store::{categories, products};
use veyndo_core::document::view;
use veyndo_core::{Result, bad, conflict};
use veyndo_shop::types::settings::Settings;

/// The JSON fields `validate_channel_config` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateChannelConfigInput {
    name: Value,
    kind: Value,
}

pub async fn validate_channel_config(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    data: &Value,
    settings: &Settings,
) -> Result<()> {
    let input: ValidateChannelConfigInput = view(data);
    let c: Channel = serde_json::from_value(data.clone()).map_err(|_| bad("Invalid channel"))?;
    if id == "default" && (c.kind != "storefront" || c.locales != settings.locales) {
        return Err(bad(
            "Main channel must remain a storefront and inherit shop languages",
        ));
    }
    if !["public", "private"].contains(&c.visibility.as_str())
        || !["storefront", "headless", "pos", "marketplace"].contains(&c.kind.as_str())
        || c.locales.is_empty()
        || c.locales.iter().any(|v| !settings.locales.contains(v))
        || c.product_ids.len() > 500
    {
        return Err(bad("Invalid channel membership"));
    }
    validate_pos(&c, settings)?;
    if let Some(root) = &c.navigation_category_id {
        let exists = categories::exists(tx, t, root).await?;
        if !exists {
            return Err(bad("Unknown channel navigation category"));
        }
    }
    for product in &c.product_ids {
        let exists = products::exists(tx, t, product).await?;
        if !exists {
            return Err(bad("Unknown channel product"));
        }
    }
    settings.languages().validate_names(&input.name, 100)?;
    let old: Option<String> = sqlx::query_scalar!(
        r#"SELECT data->>'kind' AS "kind!" FROM sales_channels WHERE tenant=$1 AND id=$2"#,
        t,
        id
    )
    .fetch_optional(&mut **tx)
    .await?;
    if old.is_some_and(|v| {
        (v == "pos") != (input.kind == "pos")
            || (v == "marketplace") != (input.kind == "marketplace")
    }) {
        return Err(conflict(
            "Operational channel type cannot be changed; create a new channel",
        ));
    }
    Ok(())
}

/// The stored document and revision of one sales channel.
pub async fn stored(a: &veyndo_core::State, t: &str, id: &str) -> Result<Option<(Value, i64)>> {
    let row = sqlx::query!(
        "SELECT data,revision FROM sales_channels WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?;
    Ok(row.map(|r| (r.data.clone(), r.revision)))
}
