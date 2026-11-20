//! Core records an app field names must belong to the same shop; no app alters core tables.
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_core::{Result, bad};

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
        let Some(id) = fields[&f.name].as_str() else {
            continue;
        };
        let query = match kind {
            "product" => "SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2)",
            "customer" => "SELECT EXISTS(SELECT 1 FROM customers WHERE tenant=$1 AND id=$2)",
            "salesChannel" => {
                "SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2)"
            }
            "order" => "SELECT EXISTS(SELECT 1 FROM orders WHERE tenant=$1 AND id=$2)",
            "b2bCompany" => "SELECT EXISTS(SELECT 1 FROM b2b_companies WHERE tenant=$1 AND id=$2)",
            "b2bUnit" => "SELECT EXISTS(SELECT 1 FROM b2b_units WHERE tenant=$1 AND id=$2)",
            _ => return Err(bad("Unknown core reference")),
        };
        let owned: bool = sqlx::query_scalar(query)
            .bind(t)
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
        if !owned {
            return Err(bad("Referenced core object does not belong to this shop"));
        }
    }
    Ok(())
}
