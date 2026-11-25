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
    sqlx::query_file!("src/store/release.sql", tenant, order)
        .execute(tx)
        .await?;
    Ok(())
}

/// Checkout already holds product locks in ID order; do not release/reacquire them here.
pub async fn consume(
    tx: &mut PgConnection,
    tenant: &str,
    lines: &InventoryLines<'_>,
) -> Result<()> {
    sqlx::query!(r#"UPDATE products p SET stock=p.stock-i.quantity,revision=p.revision+1 FROM unnest($2::text[],$3::int[]) AS i(id,quantity) WHERE p.tenant=$1 AND p.id=i.id"#, tenant, lines.ids() as &[&str], lines.quantities())
        .execute(tx)
        .await?;
    Ok(())
}
