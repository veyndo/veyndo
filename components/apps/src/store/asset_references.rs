//! Assets an app record refers to must belong to the same shop and fit the field; the catalogue answers.
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_core::{Result, bad};

/// Requires every image or file a record names to be an asset of the same shop, public where the entity is.
pub async fn references(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    e: &Entity,
    v: &Value,
) -> Result<()> {
    for f in e
        .fields
        .iter()
        .filter(|f| matches!(f.kind.as_str(), "image" | "file"))
    {
        let Some(id) = v.get(&f.name).and_then(Value::as_str) else {
            continue;
        };
        let owned = veyndo_catalog::store::app_references::asset_fits(
            &mut **tx,
            t,
            id,
            &f.kind,
            e.public_read,
        )
        .await?;
        if !owned {
            return Err(bad(
                "App asset is missing, private, or belongs to another shop",
            ));
        }
    }
    Ok(())
}
