//! Core records an app field names must belong to the same shop; the owners answer, no app alters core tables.
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_core::{Result, bad};

/// Whether a core object of the given reference kind belongs to the shop, asked of its owner on the caller's connection.
pub async fn owned(conn: &mut sqlx::PgConnection, kind: &str, t: &str, id: &str) -> Result<bool> {
    match kind {
        "product" => veyndo_catalog::store::app_references::product_exists(conn, t, id).await,
        "customer" => {
            crate::ports::directory()?
                .customer_exists(conn, t, id)
                .await
        }
        "salesChannel" => veyndo_marketing::store::app_channels::exists(conn, t, id).await,
        "order" => veyndo_orders::store::app_orders::exists(conn, t, id).await,
        "b2bCompany" => veyndo_b2b::store::app_references::company_exists(conn, t, id).await,
        "b2bUnit" => veyndo_b2b::store::app_references::unit_exists(conn, t, id).await,
        _ => Err(bad("Unknown core reference")),
    }
}

/// Requires every core reference of a record to name an object of the same shop.
pub async fn validate_references(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    e: &Entity,
    fields: &Value,
) -> Result<()> {
    for f in &e.fields {
        let Some(kind) = f.core_reference.as_deref() else {
            continue;
        };
        let Some(id) = fields.get(&f.name).and_then(Value::as_str) else {
            continue;
        };
        if !owned(tx, kind, t, id).await? {
            return Err(bad("Referenced core object does not belong to this shop"));
        }
    }
    Ok(())
}
