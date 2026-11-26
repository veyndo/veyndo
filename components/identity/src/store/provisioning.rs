//! Merchant accounts and owner memberships the server creates during operator bootstrap and shop provisioning.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The id of the personal merchant account with this email.
pub async fn account_id_by_email<'e>(
    db: impl sqlx::PgExecutor<'e>,
    email: &str,
) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar!("SELECT id FROM merchant_users WHERE email=$1", email)
            .fetch_optional(db)
            .await?,
    )
}

/// Creates a personal merchant account.
pub async fn create_account(
    conn: &mut PgConnection,
    id: &str,
    email: &str,
    name: &str,
    password_hash: &str,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO merchant_users(id,email,name,password_hash) VALUES($1,$2,$3,$4)",
        id,
        email,
        name,
        password_hash
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Grants the owner membership of a new shop.
pub async fn add_owner(conn: &mut PgConnection, user: &str, slug: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO memberships(user_id,tenant,role) VALUES($1,$2,'owner')",
        user,
        slug
    )
    .execute(conn)
    .await?;
    Ok(())
}
