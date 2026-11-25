//! The orders of a company purchasing unit and their issued receipts, as the B2B component reads them through its records port. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// An order of the unit with its procurement context.
pub struct UnitOrder {
    pub data: Value,
    pub b2b: Option<Value>,
    pub created: String,
}

/// The unit's orders, or only order `id`, newest first (at most 100).
pub async fn orders(
    conn: &mut PgConnection,
    tenant: &str,
    unit: &str,
    id: Option<&str>,
) -> Result<Vec<UnitOrder>> {
    Ok(sqlx::query_as!(UnitOrder, r#"SELECT data,data->'b2b' AS b2b,created_at::text AS "created!" FROM orders WHERE tenant=$1 AND data->'b2b'->>'unitId'=$2 AND ($3::text IS NULL OR id=$3) ORDER BY created_at DESC,id DESC LIMIT 100"#,
        tenant, unit, id)
    .fetch_all(conn)
    .await?)
}

/// An issued receipt of an order.
pub struct Receipt {
    pub id: String,
    pub kind: String,
    pub number: String,
    pub locale: String,
    pub created: String,
}

/// The receipts issued for order `id`, newest first (at most 100).
pub async fn receipts(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Vec<Receipt>> {
    Ok(sqlx::query_as!(Receipt, r#"SELECT id,kind,number,locale,created_at::text AS "created!" FROM order_receipts WHERE tenant=$1 AND order_id=$2 ORDER BY created_at DESC LIMIT 100"#,
        tenant, id)
    .fetch_all(conn)
    .await?)
}
