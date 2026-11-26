//! Shop provisioning: the server composes the starter rows of every component a new shop needs. The ID lock and the automation seed are the server's own; each row belongs to its component's store.
use serde_json::Value;
use sqlx::PgConnection;
pub(crate) use veyndo_app_platform::store::provisioning::{extension, insert_extension};
use veyndo_core::Result;
pub(crate) use veyndo_customers::store::provisioning::insert_demo_customer;
pub(crate) use veyndo_intelligence::store::provisioning::{insert_experience, insert_policy};
pub(crate) use veyndo_shop::store::provisioning::ensure_settings;

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
    veyndo_shop::store::provisioning::alias_taken(conn, slug).await
}

/// Registers the shop; `false` when the ID exists.
pub(crate) async fn create(conn: &mut PgConnection, slug: &str, name: &str) -> Result<bool> {
    veyndo_core::store::provisioning::create(conn, slug, name).await
}

/// Grants the owner membership of a new shop.
pub(crate) async fn add_owner(conn: &mut PgConnection, user: &str, slug: &str) -> Result<()> {
    veyndo_identity::store::provisioning::add_owner(conn, user, slug).await
}

/// The settings basis of a new shop.
pub(crate) async fn insert_settings(
    conn: &mut PgConnection,
    slug: &str,
    data: Value,
) -> Result<()> {
    veyndo_shop::store::provisioning::insert_settings(conn, slug, data).await
}

/// The shop's starter rules and flows (database function owned by automation).
pub(crate) async fn seed_automation<'e>(db: impl sqlx::PgExecutor<'e>, slug: &str) -> Result<()> {
    sqlx::query!("SELECT public.seed_shop_automation($1)", slug)
        .execute(db)
        .await?;
    Ok(())
}
