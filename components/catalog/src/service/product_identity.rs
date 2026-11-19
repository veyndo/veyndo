//! Saving the identity of a product: a unique number, a unique option combination per parent, channels and categories of this shop.
use crate::store::{categories, channels, product_edit};
use crate::types::product_edit::CatalogFields;
use serde_json::json;
use sqlx::PgConnection;
use veyndo_core::{Result, bad, conflict};

pub async fn save(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    fields: &CatalogFields,
) -> Result<()> {
    if product_edit::number_taken(tx, tenant, id, &fields.product_number).await? {
        return Err(conflict("Product number already exists"));
    }
    if let Some(parent) = &fields.parent_id {
        if fields.options.is_empty() {
            return Err(bad("Variant options required"));
        }
        // The product aggregate's tenant advisory lock serializes creation and editing.
        if product_edit::options_taken(tx, tenant, id, parent, &json!(fields.options)).await? {
            return Err(conflict("Variant option combination already exists"));
        }
    }
    product_edit::update_identity(tx, tenant, id, fields).await?;
    if let Some(ids) = &fields.sales_channel_ids {
        save_channels(tx, tenant, id, ids).await?;
    }
    assign_categories(tx, tenant, id, &fields.category_ids).await
}

/// Makes the product visible in exactly the given channels of this shop.
pub async fn save_channels(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    ids: &[String],
) -> Result<()> {
    if ids.len() > 100 {
        return Err(bad("Maximum 100 product channels"));
    }
    let all = channels::lock_all(tx, tenant).await?;
    if ids.iter().any(|id| !all.contains(id)) {
        return Err(bad("Unknown product sales channel"));
    }
    for channel in all {
        channels::set_visible(tx, tenant, id, &channel, ids.contains(&channel)).await?;
    }
    Ok(())
}

/// Assigns the product to exactly the given categories of this shop.
pub async fn assign_categories(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    ids: &[String],
) -> Result<()> {
    if ids.len() > 100 {
        return Err(bad("Maximum 100 product categories"));
    }
    for category in ids {
        if !categories::exists(tx, tenant, category).await? {
            return Err(bad("Category unavailable in this shop"));
        }
    }
    categories::replace(tx, tenant, id, ids).await
}
