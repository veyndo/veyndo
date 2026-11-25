//! The shop row of a new sandbox, created inside the caller's environment transaction.
use crate::Result;

/// Creates the shop row of a sandbox.
pub async fn insert_shop(c: &mut sqlx::PgConnection, id: &str, name: &str) -> Result<()> {
    sqlx::query!("INSERT INTO tenants(id,name) VALUES($1,$2)", id, name)
        .execute(&mut *c)
        .await?;
    Ok(())
}
