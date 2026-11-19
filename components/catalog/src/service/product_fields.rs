//! Saving the commerce fields of a product after its 3D models were checked against the stored assets.
use crate::service::model_media;
use crate::store::product_fields;
use crate::types::product_fields::ProductFields;
use sqlx::PgConnection;
use veyndo_core::Result;

pub async fn save(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    fields: &ProductFields,
) -> Result<()> {
    model_media::validate(tx, tenant, id, &fields.media).await?;
    product_fields::save(tx, tenant, id, fields).await
}
