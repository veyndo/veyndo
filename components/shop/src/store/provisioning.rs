//! Shop rows the server writes during provisioning and operator bootstrap: the settings basis, the hosted-address check, the operator grant and the platform audit.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Whether a hosted frontend already uses this address.
pub async fn alias_taken(conn: &mut PgConnection, slug: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE alias=$1) AS "taken!""#,
        slug
    )
    .fetch_one(conn)
    .await?)
}

/// The settings basis of a new shop.
pub async fn insert_settings(conn: &mut PgConnection, slug: &str, data: Value) -> Result<()> {
    sqlx::query!(
        "INSERT INTO commerce_settings(tenant,data) VALUES($1,$2)",
        slug,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// The settings basis of a built-in demo shop, unless the shop has settings.
pub async fn ensure_settings<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
    data: Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO commerce_settings(tenant,data) VALUES($1,$2) ON CONFLICT DO NOTHING",
        slug,
        data
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Grants (or re-activates) the operator role.
pub async fn grant_operator(conn: &mut PgConnection, user: &str) -> Result<()> {
    sqlx::query!("INSERT INTO platform_operators(user_id) VALUES($1) ON CONFLICT(user_id) DO UPDATE SET active=true", user)
        .execute(conn)
        .await?;
    Ok(())
}

/// Records the operator bootstrap.
pub async fn audit_operator_bootstrap(conn: &mut PgConnection, user: &str) -> Result<()> {
    sqlx::query!("INSERT INTO platform_audit(actor,action,data) VALUES($1,'operator.bootstrap',jsonb_build_object('userId',$1::text))", user)
        .execute(conn)
        .await?;
    Ok(())
}

/// Records an operator-created shop.
pub async fn audit_shop_created(
    conn: &mut PgConnection,
    actor: &str,
    tenant: &str,
    data: Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO platform_audit(actor,action,tenant,data) VALUES($1,'shop.created',$2,$3)",
        actor,
        tenant,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}
