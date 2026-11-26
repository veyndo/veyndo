//! The synthetic demo buyer a provisioned or seeded demo shop starts with.
use veyndo_core::Result;

/// The demo business customer `buyer@example.test`; `keep_existing` leaves a present one untouched.
pub async fn insert_demo_customer<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
    password_hash: &str,
    keep_existing: bool,
) -> Result<()> {
    if keep_existing {
        sqlx::query!("INSERT INTO customers(tenant,email,password_hash,company,group_name) VALUES($1,'buyer@example.test',$2,'Example Studio','business') ON CONFLICT DO NOTHING", slug, password_hash)
            .execute(db)
            .await?;
    } else {
        sqlx::query!("INSERT INTO customers(tenant,email,password_hash,company,group_name) VALUES($1,'buyer@example.test',$2,'Example Studio','business')", slug, password_hash)
            .execute(db)
            .await?;
    }
    Ok(())
}
