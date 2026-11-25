//! Per-product channel visibility overrides remain indexed even when an open catalog contains millions of products.
use serde::Serialize;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The IDs of all sales channels of the shop, locked so visibility is written against a stable set.
pub async fn lock_all(tx: &mut PgConnection, tenant: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT id FROM sales_channels WHERE tenant=$1 ORDER BY id FOR UPDATE"#,
        tenant
    )
    .fetch_all(tx)
    .await?)
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
    Ok(sqlx::query_as!(Visibility, r#"SELECT c.id,c.data,coalesce(v.visible,true) AS "visible!" FROM sales_channels c LEFT JOIN product_channel_visibility v ON v.tenant=c.tenant AND v.channel_id=c.id AND v.product_id=$2 WHERE c.tenant=$1 ORDER BY c.id LIMIT 101"#, tenant, product)
        .fetch_all(&a.db)
        .await?)
}
