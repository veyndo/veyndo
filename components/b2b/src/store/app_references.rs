//! Whether a company or purchasing unit an app record or surface context names belongs to the shop.
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

/// Whether the shop has the company.
pub async fn company_exists<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    id: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM b2b_companies WHERE tenant=$1 AND id=$2) AS "owned!""#,
        t,
        id
    )
    .fetch_one(db)
    .await?)
}

/// Whether the shop has the purchasing unit.
pub async fn unit_exists<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    id: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM b2b_units WHERE tenant=$1 AND id=$2) AS "owned!""#,
        t,
        id
    )
    .fetch_one(db)
    .await?)
}
