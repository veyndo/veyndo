//! The migration ledger and runner statements: trusted context, the session lock, ledger reads and writes, applying a migration script and the demo-seed marker. Static statements are checked at compile time.
use sqlx::{PgConnection, PgPool};
use veyndo_core::Result;

/// Marks the setup session as the trusted migration context.
pub(crate) async fn enter_trusted(pool: &PgPool) -> Result<()> {
    sqlx::query_scalar!("SELECT set_config('rac.system','on',false)")
        .fetch_one(pool)
        .await?;
    Ok(())
}

/// The session-wide migration lock; the one-connection setup pool keeps it through setup and seed.
pub(crate) async fn lock(pool: &PgPool) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_lock(7193511)")
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn ensure_ledger(pool: &PgPool) -> Result<()> {
    sqlx::query!("CREATE TABLE IF NOT EXISTS public.commerce_migrations(version text PRIMARY KEY,checksum text NOT NULL,applied_at timestamptz NOT NULL DEFAULT now())")
        .execute(pool)
        .await?;
    Ok(())
}

/// The checksum recorded for an applied migration, if it is applied.
pub(crate) async fn applied_checksum<'e>(
    db: impl sqlx::PgExecutor<'e>,
    version: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT checksum FROM public.commerce_migrations WHERE version=$1",
        version
    )
    .fetch_optional(db)
    .await?)
}

/// Runs one migration script in the public schema of the open transaction.
pub(crate) async fn run_script(conn: &mut PgConnection, source: &'static str) -> Result<()> {
    sqlx::query!("SET LOCAL search_path=public")
        .execute(&mut *conn)
        .await?;
    // A migration script is many statements, applied as written: no single query to check.
    sqlx::raw_sql(source).execute(conn).await?;
    Ok(())
}

/// Runs a bounded built-in demo metadata script.
pub(crate) async fn run_fixture<'e>(
    db: impl sqlx::PgExecutor<'e>,
    source: &'static str,
) -> Result<()> {
    // A fixture script is many statements, applied as written: no single query to check.
    sqlx::raw_sql(source).execute(db).await?;
    Ok(())
}

pub(crate) async fn record(conn: &mut PgConnection, version: &str, checksum: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO public.commerce_migrations(version,checksum) VALUES($1,$2)",
        version,
        checksum
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Whether the built-in demo seed already ran.
pub(crate) async fn demo_seeded<'e>(db: impl sqlx::PgExecutor<'e>) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM public.commerce_migrations WHERE version='demo-v1') AS "seeded!""#
    )
    .fetch_one(db)
    .await?)
}

pub(crate) async fn record_demo_seed(pool: &PgPool) -> Result<()> {
    sqlx::query!("INSERT INTO public.commerce_migrations(version,checksum) VALUES('demo-v1','bounded-built-in-fixtures')")
        .execute(pool)
        .await?;
    Ok(())
}
