//! Barcode lookup of active products by id, product number or EAN. Columns checked at compile time.
use veyndo_core::{Result, State};

/// Active products whose id, number or EAN equals `barcode` (at most two, so ambiguity shows).
pub async fn barcode(a: &State, tenant: &str, barcode: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM products WHERE tenant=$1 AND active AND (id=$2 OR product_number=$2 OR extra->>'ean'=$2) ORDER BY id LIMIT 2",
        tenant,
        barcode
    )
    .fetch_all(&a.db)
    .await?)
}
