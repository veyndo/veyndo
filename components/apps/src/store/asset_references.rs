//! Assets an app record refers to must belong to the same shop and fit the field.
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
        let Some(id) = v[&f.name].as_str() else {
            continue;
        };
        let owned = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM product_assets WHERE tenant=$1 AND id=$2 AND ($3::text<> 'image' OR mime LIKE 'image/%') AND (NOT $4::bool OR public AND kind='attachment')) AS "owned!""#,
            t,
            id,
            f.kind,
            e.public_read
        )
        .fetch_one(&mut **tx)
        .await?;
        if !owned {
            return Err(bad(
                "App asset is missing, private, or belongs to another shop",
            ));
        }
    }
    Ok(())
}
