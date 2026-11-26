//! Model media references reuse owned immutable product assets, current product CAS and parent variant links.
use crate::domain::{media, model_format};
use crate::store::assets;
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};

/// The fields of a media item that name a 3D model and its material variant.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ModelItem {
    #[serde(deserialize_with = "lenient")]
    r#type: Option<String>,
    #[serde(deserialize_with = "lenient")]
    url: Option<String>,
    #[serde(deserialize_with = "lenient")]
    model_variant: Option<String>,
}

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
        .map(view::<ModelItem>)
        .filter(|m| m.r#type.as_deref() == Some("model"))
    {
        let id = media::model_asset(item.url.as_deref().unwrap_or_default(), tenant)?;
        let prefix = assets::published_model_prefix(tx, tenant, &id, product)
            .await?
            .ok_or(bad(
                "Published GLB asset for this product or its parent required",
            ))?;
        let doc = model_format::document(&prefix)?;
        if let Some(name) = item.model_variant.filter(|s| !s.is_empty())
            && !model_format::variants(&doc)?.contains(&name)
        {
            return Err(bad("Material variant is not present in this GLB"));
        }
    }
    Ok(())
}
