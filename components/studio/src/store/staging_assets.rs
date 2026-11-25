//! Binary assets are immutable, staged independently through metadata/digest units; paid entitlements never clone.
use super::staging_documents::Tx;
use crate::domain::views::Identified;
use crate::domain::views::MediaUrl;
use serde_json::{Map, Value};
use veyndo_catalog::store::staging as catalog;
use veyndo_core::document::view;
use veyndo_core::{Result, bad, conflict};

/// Adds every asset as a release unit by metadata and digest, never by content.
pub async fn snapshot_assets(
    tx: &mut Tx<'_>,
    t: &str,
    result: &mut Map<String, Value>,
) -> Result<()> {
    let rows = catalog::assets(tx, t).await?;
    for v in rows {
        result.insert(format!("asset:{}", view::<Identified>(&v).id.unwrap()), v);
    }
    Ok(())
}
/// Publishes one asset; its product must already exist in the live shop.
pub async fn publish_asset(tx: &mut Tx<'_>, live: &str, stage: &str, id: &str) -> Result<()> {
    let product = catalog::asset_product(tx, live, stage, id).await?;
    if product.is_none() {
        return Err(conflict("Publish this asset together with its product"));
    }
    catalog::publish_asset(tx, live, stage, id).await?;
    Ok(())
}

/// A selected product must not publish an image URL whose private bytes were omitted from the same release.
pub async fn validate_product_media(tx: &mut Tx<'_>, live: &str, keys: &[String]) -> Result<()> {
    for id in keys.iter().filter_map(|s| s.strip_prefix("product:")) {
        let media = catalog::product_media(tx, live, id).await?;
        for item in media
            .as_array()
            .ok_or(bad("Product media must be an array"))?
        {
            let Some(url) = view::<MediaUrl>(item)
                .url
                .filter(|s| s.starts_with("/store-api/assets/"))
            else {
                continue;
            };
            let url = reqwest::Url::parse(&format!("https://local{url}"))
                .map_err(|_| bad("Invalid local asset URL"))?;
            let asset = url
                .path()
                .strip_prefix("/store-api/assets/")
                .filter(|s| !s.is_empty() && s.len() <= 100)
                .ok_or(bad("Invalid local asset identity"))?;
            let exists = catalog::public_attachment(tx, live, asset).await?;
            if exists != Some(true) {
                return Err(conflict(&format!(
                    "Product {id} needs its image asset {asset}; include that asset in this release"
                )));
            }
        }
    }
    Ok(())
}
