//! The business policy digest a purchase approves against, share-locked on the purchase connection.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The tenant's business policy module digest, share-locked until the purchase transaction ends.
pub async fn locked_digest(conn: &mut PgConnection, tenant: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(
        "SELECT digest FROM extensions WHERE tenant=$1 FOR SHARE",
        tenant
    )
    .fetch_one(conn)
    .await?)
}
