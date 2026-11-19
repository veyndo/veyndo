//! Model media references reuse owned immutable product assets, current product CAS and parent variant links.
use crate::domain::{media, model_format};
use crate::store::assets;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, bad};

/// Every 3D model of `media` is a published GLB of this product or its parent and contains the chosen material variant.
pub async fn validate(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
    media: &Value,
) -> Result<()> {
    for item in media
        .as_array()
        .ok_or(bad("Invalid product media"))?
        .iter()
        .filter(|m| m["type"] == "model")
    {
        let id = media::model_asset(item["url"].as_str().unwrap_or_default(), tenant)?;
        let prefix = assets::published_model_prefix(tx, tenant, &id, product)
            .await?
            .ok_or(bad(
                "Published GLB asset for this product or its parent required",
            ))?;
        let doc = model_format::document(&prefix)?;
        if let Some(name) = item["modelVariant"].as_str().filter(|s| !s.is_empty())
            && !model_format::variants(&doc)?.iter().any(|s| s == name)
        {
            return Err(bad("Material variant is not present in this GLB"));
        }
    }
    Ok(())
}
