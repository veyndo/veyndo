//! Reads of stored product assets.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The first bytes (header and metadata) of a published GLB that belongs to the product or its parent.
pub async fn published_model_prefix(
    tx: &mut PgConnection,
    tenant: &str,
    asset: &str,
    product: &str,
) -> Result<Option<Vec<u8>>> {
    Ok(sqlx::query_scalar("SELECT substring(content for 1048596) AS prefix FROM product_assets WHERE tenant=$1 AND id=$2 AND public AND kind='attachment' AND mime='model/gltf-binary' AND (product_id=$3 OR product_id=(SELECT parent_id FROM products WHERE tenant=$1 AND id=$3)) FOR SHARE")
        .bind(tenant)
        .bind(asset)
        .bind(product)
        .fetch_optional(tx)
        .await?)
}
