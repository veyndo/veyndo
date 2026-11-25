//! Per-product channel visibility overrides remain indexed even when an open catalog contains millions of products. The channels themselves come through the registries port.
use serde::Serialize;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The IDs of all sales channels of the shop, locked so visibility is written against a stable set.
pub async fn lock_all(tx: &mut PgConnection, tenant: &str) -> Result<Vec<String>> {
    crate::ports::registries()?.lock_channels(tx, tenant).await
}

pub async fn set_visible(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
    channel: &str,
    visible: bool,
) -> Result<()> {
    sqlx::query!(r#"INSERT INTO product_channel_visibility VALUES($1,$2,$3,$4) ON CONFLICT(tenant,product_id,channel_id) DO UPDATE SET visible=EXCLUDED.visible"#, tenant, product, channel, visible)
        .execute(tx)
        .await?;
    Ok(())
}

/// A sales channel and whether the product is visible in it; visible unless overridden.
#[derive(Serialize, sqlx::FromRow)]
pub struct Visibility {
    pub id: String,
    pub data: Value,
    pub visible: bool,
}

/// Up to 101 channels, so the caller can refuse shops beyond the editor's limit of 100.
pub async fn visibility(a: &State, tenant: &str, product: &str) -> Result<Vec<Visibility>> {
    let channels = crate::ports::registries()?.channels(tenant, 101).await?;
    let ids: Vec<&str> = channels.iter().map(|(id, _)| id.as_str()).collect();
    let hidden = sqlx::query_scalar!(r#"SELECT channel_id FROM product_channel_visibility WHERE tenant=$1 AND product_id=$2 AND channel_id=ANY($3) AND NOT visible"#, tenant, product, &ids as &[&str])
        .fetch_all(&a.db)
        .await?;
    Ok(channels
        .into_iter()
        .map(|(id, data)| Visibility {
            visible: !hidden.contains(&id),
            id,
            data,
        })
        .collect())
}
