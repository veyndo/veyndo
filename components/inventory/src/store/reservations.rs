//! All-checkout allocation ledger; release is idempotent and always uses persisted quantities, never edited order JSON.
use crate::domain::lines::InventoryLines;
use sqlx::PgConnection;
use veyndo_core::Result;

pub async fn allocate(
    tx: &mut PgConnection,
    tenant: &str,
    lines: &InventoryLines<'_>,
    order: &str,
) -> Result<()> {
    sqlx::query!(r#"INSERT INTO order_inventory_reservations(tenant,order_id,product_id,quantity) SELECT $1,$2,id,quantity FROM unnest($3::text[],$4::int[]) AS items(id,quantity)"#, tenant, order, lines.ids() as &[&str], lines.quantities())
        .execute(tx)
        .await?;
    Ok(())
}

pub async fn release(tx: &mut PgConnection, tenant: &str, order: &str) -> Result<()> {
    // Persisted quantities only; releasing twice yields no changes.
    let released = sqlx::query!(
        "UPDATE order_inventory_reservations SET released_at=now() WHERE tenant=$1 AND order_id=$2 AND released_at IS NULL RETURNING product_id,quantity",
        tenant,
        order
    )
    .fetch_all(&mut *tx)
    .await?;
    if released.is_empty() {
        return Ok(());
    }
    let (ids, quantities): (Vec<String>, Vec<i32>) = released
        .into_iter()
        .map(|row| (row.product_id, row.quantity))
        .unzip();
    // Lock affected products in checkout's ID order.
    veyndo_catalog::store::stock::put_back(tx, tenant, &ids, &quantities).await
}

/// Checkout already holds product locks in ID order; do not release/reacquire them here.
pub async fn consume(
    tx: &mut PgConnection,
    tenant: &str,
    lines: &InventoryLines<'_>,
) -> Result<()> {
    veyndo_catalog::store::stock::take(tx, tenant, lines.ids(), lines.quantities()).await
}
