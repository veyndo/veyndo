//! Shop registry rows written when the server provisions a new shop.
use crate::Result;
use sqlx::PgConnection;

/// Registers the shop; `false` when the ID exists.
pub async fn create(conn: &mut PgConnection, slug: &str, name: &str) -> Result<bool> {
    Ok(sqlx::query!(
        "INSERT INTO tenants(id,name) VALUES($1,$2) ON CONFLICT DO NOTHING",
        slug,
        name
    )
    .execute(conn)
    .await?
    .rows_affected()
        > 0)
}
