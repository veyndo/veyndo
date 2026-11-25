//! The runtime's SQL: the database scope functions (key, ticket, configure, activate, rotate, status). Pools and signing stay in their modules.
use sqlx::{PgConnection, PgPool, Postgres};

/// The tenant login role of the current scope key.
pub async fn scope_login<'e>(
    db: impl sqlx::Executor<'e, Database = Postgres>,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT tenant_login::text AS "login!" FROM public.veyndo_scope_key()"#)
        .fetch_one(db)
        .await
}

/// Binds the signed scope ticket to the open transaction; a bound parameter keeps it out of `pg_stat_activity`.
pub async fn bind_ticket(conn: &mut PgConnection, ticket: String) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!("SELECT set_config('veyndo.scope_ticket',$1,true)", ticket)
        .fetch_one(conn)
        .await?;
    Ok(())
}

pub async fn configure_scope(pool: &PgPool, system: &str, tenant: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "SELECT public.veyndo_configure_scope($1::name,$2::name)",
        system,
        tenant
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn activate_scope(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!("SELECT public.veyndo_activate_scope()")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rotate_scope_key(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!("SELECT public.veyndo_rotate_scope_key()")
        .fetch_one(pool)
        .await?;
    Ok(())
}

pub async fn scope_status(pool: &PgPool) -> Result<serde_json::Value, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT public.veyndo_scope_status() AS "status!""#)
        .fetch_one(pool)
        .await
}
