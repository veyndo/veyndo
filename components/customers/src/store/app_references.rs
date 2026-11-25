//! Whether a customer an app record or surface context names belongs to the shop.
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

/// Whether the shop has the customer.
pub async fn exists<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    id: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM customers WHERE tenant=$1 AND id=$2) AS "owned!""#,
        t,
        id
    )
    .fetch_one(db)
    .await?)
}
