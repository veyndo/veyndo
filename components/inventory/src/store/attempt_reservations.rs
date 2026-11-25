//! Stock a pending provider payment attempt holds, written on the payment ledger's connection.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Reserves one line of stock for the pending attempt.
pub async fn reserve(
    conn: &mut PgConnection,
    tenant: &str,
    attempt: &str,
    product: &str,
    quantity: i32,
) -> Result<()> {
    sqlx::query!("INSERT INTO inventory_reservations(tenant,attempt_id,product_id,quantity) VALUES($1,$2,$3,$4)", tenant, attempt, product, quantity)
        .execute(conn)
        .await?;
    Ok(())
}

/// Releases every reservation of the attempt that is still held.
pub async fn release_attempt(conn: &mut PgConnection, tenant: &str, attempt: &str) -> Result<()> {
    sqlx::query!("UPDATE inventory_reservations SET released=true WHERE tenant=$1 AND attempt_id=$2 AND NOT released", tenant, attempt)
        .execute(conn)
        .await?;
    Ok(())
}
