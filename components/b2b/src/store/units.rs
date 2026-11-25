//! What an employee sees of their purchasing units: active memberships with unit and company (checked columns), and the unit's orders with their issued receipts, which belong to orders and are read through the records port.
use crate::ports::records;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// An active unit the customer belongs to, with its company and membership.
pub struct Membership {
    pub id: String,
    pub company_id: String,
    pub data: Value,
    pub company: Value,
    pub member: Value,
}

/// The customer's active memberships in active units of active companies, by unit (at most 100).
pub async fn memberships(
    conn: &mut PgConnection,
    tenant: &str,
    customer: &str,
) -> Result<Vec<Membership>> {
    Ok(sqlx::query_as!(Membership, "SELECT u.id,u.company_id,u.data,c.data AS company,m.data AS member FROM b2b_members m JOIN b2b_units u ON u.tenant=m.tenant AND u.id=m.unit_id JOIN b2b_companies c ON c.tenant=u.tenant AND c.id=u.company_id WHERE m.tenant=$1 AND m.customer_id=$2 AND m.data->>'active'='true' AND u.data->>'active'='true' AND c.data->>'active'='true' ORDER BY u.id LIMIT 100",
        tenant, customer)
    .fetch_all(conn)
    .await?)
}

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
    records()?.unit_orders(conn, tenant, unit, id).await
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
    records()?.receipts(conn, tenant, id).await
}
