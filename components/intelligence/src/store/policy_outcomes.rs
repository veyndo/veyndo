//! Purchase outcomes of the experiment policy, counted inside the transaction that placed or captured the order.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Counts one purchase for the variant.
pub async fn count_purchase(conn: &mut PgConnection, tenant: &str, variant: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE policy SET purchases=purchases+1 WHERE tenant=$1 AND variant=$2",
        tenant,
        variant
    )
    .execute(conn)
    .await?;
    Ok(())
}
