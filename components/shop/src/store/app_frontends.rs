//! The hosted-frontend address registry as the app platform uses it: the apps frontends depend on, the owning shop's bindings and their revisioned writes.
use sqlx::{PgConnection, Postgres, Transaction};
use veyndo_core::{Result, State};

/// Whether a connected frontend of the shop depends on the app.
pub async fn app_in_use(conn: &mut PgConnection, t: &str, app: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE tenant=$1 AND app_id=$2) AS "used!""#,
        t,
        app
    )
    .fetch_one(conn)
    .await?)
}

/// Every shop with a frontend of the hosted storefront experience, for the migration backfill.
pub async fn storyfront_tenants(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT DISTINCT tenant FROM hosted_frontends WHERE app_id='storyfront' ORDER BY tenant"
    )
    .fetch_all(&mut **tx)
    .await?)
}

/// Validates the frontend-to-app reference once every dependent app is installed.
pub async fn validate_app_reference(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    sqlx::query!("ALTER TABLE hosted_frontends VALIDATE CONSTRAINT hosted_frontends_app_fk")
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// One connected hosted frontend of the shop.
pub struct Connection {
    pub alias: String,
    pub channel: String,
    pub experience: String,
    pub revision: i64,
    pub app: Option<String>,
}

/// The hosted frontends of the shop, by alias.
pub async fn connections(a: &State, t: &str) -> Result<Vec<Connection>> {
    Ok(sqlx::query_as!(
        Connection,
        r#"SELECT alias,channel,experience_alias AS experience,revision,app_id AS app FROM hosted_frontends WHERE tenant=$1 ORDER BY alias"#,
        t
    )
    .fetch_all(&a.db)
    .await?)
}

/// The app of the shop's first frontend that mounts an Experience: `None` when the shop does not mount it, `Some(None)` when it mounts it without an app.
pub async fn experience_app(
    conn: &mut PgConnection,
    t: &str,
    experience: &str,
) -> Result<Option<Option<String>>> {
    Ok(sqlx::query_scalar!(
        "SELECT app_id FROM hosted_frontends WHERE tenant=$1 AND experience_alias=$2 ORDER BY alias LIMIT 1",
        t,
        experience
    )
    .fetch_optional(conn)
    .await?)
}

/// A stored frontend address.
pub struct Binding {
    pub tenant: String,
    pub channel: String,
    pub experience_alias: String,
    pub revision: i64,
    pub app_id: Option<String>,
}

/// Locks an address in any shop.
pub async fn lock(conn: &mut PgConnection, alias: &str) -> Result<Option<Binding>> {
    Ok(sqlx::query_as!(
        Binding,
        "SELECT tenant,channel,experience_alias,revision,app_id FROM hosted_frontends WHERE alias=$1 FOR UPDATE",
        alias
    )
    .fetch_optional(conn)
    .await?)
}

/// A frontend address to bind.
pub struct NewBinding<'a> {
    pub alias: &'a str,
    pub tenant: &'a str,
    pub channel: &'a str,
    pub origin: &'a str,
    pub experience: &'a str,
    pub app_id: Option<&'a str>,
}

/// Binds an address or changes the shop's binding, raising its revision; returns the revision. An address of another shop fails.
pub async fn save(conn: &mut PgConnection, b: NewBinding<'_>) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        "INSERT INTO hosted_frontends(alias,tenant,channel,origin,experience_alias,app_id) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(alias) DO UPDATE SET channel=excluded.channel,origin=excluded.origin,experience_alias=excluded.experience_alias,app_id=excluded.app_id,revision=hosted_frontends.revision+1 WHERE hosted_frontends.tenant=excluded.tenant RETURNING revision",
        b.alias,
        b.tenant,
        b.channel,
        b.origin,
        b.experience,
        b.app_id
    )
    .fetch_one(conn)
    .await?)
}

/// Removes an address of the shop at its revision; returns the rows removed.
pub async fn remove(conn: &mut PgConnection, t: &str, alias: &str, revision: i64) -> Result<u64> {
    Ok(sqlx::query!(
        "DELETE FROM hosted_frontends WHERE tenant=$1 AND alias=$2 AND revision=$3",
        t,
        alias,
        revision
    )
    .execute(conn)
    .await?
    .rows_affected())
}
