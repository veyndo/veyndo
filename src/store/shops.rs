//! Shop provisioning writes: the registry row, the owner membership and the starter rows of every component a new shop needs. The server composes them; columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Serializes provisioning of one shop ID.
pub(crate) async fn lock_id(conn: &mut PgConnection, slug: &str) -> Result<()> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,43))",
        slug
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Whether a hosted frontend already uses this address.
pub(crate) async fn alias_taken(conn: &mut PgConnection, slug: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE alias=$1) AS "taken!""#,
        slug
    )
    .fetch_one(conn)
    .await?)
}

/// Registers the shop; `false` when the ID exists.
pub(crate) async fn create(conn: &mut PgConnection, slug: &str, name: &str) -> Result<bool> {
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

pub(crate) async fn add_owner(conn: &mut PgConnection, user: &str, slug: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO memberships(user_id,tenant,role) VALUES($1,$2,'owner')",
        user,
        slug
    )
    .execute(conn)
    .await?;
    Ok(())
}

pub(crate) async fn insert_settings(
    conn: &mut PgConnection,
    slug: &str,
    data: Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO commerce_settings(tenant,data) VALUES($1,$2)",
        slug,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// The starter storefront experience; `keep_existing` leaves a present one untouched.
pub(crate) async fn insert_experience<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
    data: Value,
    keep_existing: bool,
) -> Result<()> {
    if keep_existing {
        sqlx::query!(
            "INSERT INTO experiences(tenant,data) VALUES($1,$2) ON CONFLICT DO NOTHING",
            slug,
            data
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO experiences(tenant,data) VALUES($1,$2)",
            slug,
            data
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

/// A starter policy variant; `keep_existing` leaves a present one untouched.
pub(crate) async fn insert_policy<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
    variant: &str,
    keep_existing: bool,
) -> Result<()> {
    if keep_existing {
        sqlx::query!(
            "INSERT INTO policy(tenant,variant) VALUES($1,$2) ON CONFLICT DO NOTHING",
            slug,
            variant
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO policy(tenant,variant) VALUES($1,$2)",
            slug,
            variant
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

/// The settings basis of a built-in demo shop, unless the shop has settings.
pub(crate) async fn ensure_settings<'e>(
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

/// The shop's starter rules and flows (database function owned by automation).
pub(crate) async fn seed_automation<'e>(db: impl sqlx::PgExecutor<'e>, slug: &str) -> Result<()> {
    sqlx::query!("SELECT public.seed_shop_automation($1)", slug)
        .execute(db)
        .await?;
    Ok(())
}

/// The demo business customer `buyer@example.test`; `keep_existing` leaves a present one untouched.
pub(crate) async fn insert_demo_customer<'e>(
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

/// The shop's starter Wasm extension; `keep_existing` leaves a present one untouched.
pub(crate) async fn insert_extension<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
    wat: &str,
    digest: &str,
    keep_existing: bool,
) -> Result<()> {
    if keep_existing {
        sqlx::query!(
            "INSERT INTO extensions(tenant,wat,digest) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
            slug,
            wat,
            digest
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO extensions(tenant,wat,digest) VALUES($1,$2,$3)",
            slug,
            wat,
            digest
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

/// The saved Wasm extension of a shop, if any.
pub(crate) async fn extension<'e>(
    db: impl sqlx::PgExecutor<'e>,
    slug: &str,
) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar!("SELECT wat FROM extensions WHERE tenant=$1", slug)
            .fetch_optional(db)
            .await?,
    )
}
