//! Release provisioning on the migration-owner connection (`--provision`, and the grant part after every migration): grants every schema object to the existing runtime group and connector login, configures and activates database authority, verifies the result and optionally bootstraps the first operator. It never creates logins or passwords; those belong to the database manager.
use crate::store::provision::{self as store, Coverage};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::env;
use veyndo::scoped_pool::ScopedPool;
use veyndo_runtime::store as authority;

type Failure = Box<dyn std::error::Error>;

/// Role names from the environment. A name that was explicitly configured must exist; a default
/// name that does not exist is skipped with a note (a development database may have no such role).
struct Roles {
    runtime: (String, bool),
    connector: (String, bool),
}

fn named(key: &str, default: &str) -> Result<(String, bool), Failure> {
    match env::var(key) {
        Ok(value) if value.trim().is_empty() => Err(format!("{key} is empty").into()),
        Ok(value) => Ok((value.trim().to_owned(), true)),
        Err(_) => Ok((default.to_owned(), false)),
    }
}

impl Roles {
    fn from_env() -> Result<Self, Failure> {
        Ok(Self {
            runtime: named("DB_RUNTIME_ROLE", "veyndo_runtime")?,
            connector: named("DB_CONNECTOR_LOGIN", "veyndo_connectors")?,
        })
    }
}

fn complete(what: &str, role: &str, c: &Coverage) -> Result<(), Failure> {
    println!(
        "{what} {role}: {}/{} tables, {}/{} sequences",
        c.tables - c.missing_tables,
        c.tables,
        c.sequences - c.missing_sequences,
        c.sequences
    );
    if c.missing_tables > 0 || c.missing_sequences > 0 {
        return Err(format!(
            "{what} {role} lacks privileges on {} tables and {} sequences",
            c.missing_tables, c.missing_sequences
        )
        .into());
    }
    if !c.connect || !c.schema_usage {
        return Err(format!(
            "{what} {role} needs CONNECT on the database and USAGE on schema public; the database manager grants these"
        )
        .into());
    }
    Ok(())
}

/// Grants the runtime group and connector login on every current object and verifies them.
/// `strict` turns a missing role into an error even when its name is the default.
async fn grant(db: &PgPool, roles: &Roles, strict: bool) -> Result<(), Failure> {
    let (runtime, explicit) = &roles.runtime;
    if store::role_exists(db, runtime).await? {
        store::execute(db, &store::runtime_statements(db, runtime).await?).await?;
        complete(
            "Runtime role",
            runtime,
            &store::runtime_coverage(db, runtime).await?,
        )?;
    } else if strict || *explicit {
        return Err(format!(
            "Runtime role {runtime} does not exist; the database manager creates it (DB_RUNTIME_ROLE)"
        )
        .into());
    } else {
        println!("Runtime role {runtime} does not exist; runtime grants skipped");
    }
    let (connector, explicit) = &roles.connector;
    if store::role_exists(db, connector).await? {
        store::execute(db, &store::connector_statements(db, connector).await?).await?;
        complete(
            "Connector login",
            connector,
            &store::connector_coverage(db, connector).await?,
        )?;
    } else if *explicit {
        return Err(format!(
            "Connector login {connector} does not exist; the database manager creates it (DB_CONNECTOR_LOGIN)"
        )
        .into());
    } else {
        println!("Connector login {connector} does not exist; connector grants skipped");
    }
    Ok(())
}

fn text(status: &serde_json::Value, key: &str) -> Option<String> {
    status[key].as_str().map(str::to_owned)
}

/// Re-applies an existing authority configuration so the tenant login reaches new tables. A
/// staged configuration stays staged: only `--provision` with named logins activates.
async fn refresh_authority(db: &PgPool) -> Result<Option<(String, String)>, Failure> {
    let status = authority::scope_status(db).await?;
    let (Some(system), Some(tenant)) = (text(&status, "systemRole"), text(&status, "tenantRole"))
    else {
        return Ok(None);
    };
    authority::configure_scope(db, &system, &tenant).await?;
    Ok(Some((system, tenant)))
}

async fn verify_authority(db: &PgPool, tenant: &str) -> Result<(), Failure> {
    let c = store::authority_coverage(db, tenant).await?;
    println!(
        "Tenant login {tenant}: {}/{} scoped tables",
        c.tenant_tables - c.tenant_missing,
        c.tenant_tables
    );
    if c.tenant_missing > 0 {
        return Err(format!(
            "Tenant login {tenant} lacks {} scoped tables",
            c.tenant_missing
        )
        .into());
    }
    if c.tenant_temporary {
        return Err(format!(
            "Tenant login {tenant} can create temporary objects; the database owner must revoke TEMPORARY on the database from PUBLIC"
        )
        .into());
    }
    Ok(())
}

/// The grant part of provisioning, run by every migration so new tables of a release are granted.
pub(crate) async fn after_migrate(db: &PgPool) -> Result<(), Failure> {
    grant(db, &Roles::from_env()?, false).await?;
    if let Some((_, tenant)) = refresh_authority(db).await? {
        verify_authority(db, &tenant).await?;
    }
    Ok(())
}

async fn provision(db: &PgPool) -> Result<(), Failure> {
    crate::migrations::ready(&ScopedPool::new(db.clone(), false)).await;
    let roles = Roles::from_env()?;
    grant(db, &roles, true).await?;
    let system = env::var("DB_SYSTEM_LOGIN").ok().filter(|s| !s.is_empty());
    let tenant = env::var("DB_TENANT_LOGIN").ok().filter(|s| !s.is_empty());
    let (system, tenant) = match (system, tenant) {
        (Some(system), Some(tenant)) => {
            authority::configure_scope(db, &system, &tenant).await?;
            authority::activate_scope(db).await?;
            (system, tenant)
        }
        (None, None) => match refresh_authority(db).await? {
            Some(roles) => roles,
            None => {
                println!(
                    "Database authority not configured (DB_SYSTEM_LOGIN and DB_TENANT_LOGIN unset)"
                );
                return Ok(());
            }
        },
        _ => return Err("Set both DB_SYSTEM_LOGIN and DB_TENANT_LOGIN, or neither".into()),
    };
    // The system login serves native identity and workers: it must reach every Core table,
    // normally through membership in the runtime role.
    complete(
        "System login",
        &system,
        &store::runtime_coverage(db, &system).await?,
    )?;
    verify_authority(db, &tenant).await?;
    println!("Database authority: {}", authority::scope_status(db).await?);
    Ok(())
}

/// `veyndo --provision`: run with `BOOTSTRAP_MODE=migrate` on the migration-owner connection.
pub(crate) async fn command() {
    assert_eq!(
        env::var("BOOTSTRAP_MODE").as_deref(),
        Ok("migrate"),
        "Provisioning requires BOOTSTRAP_MODE=migrate"
    );
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL required");
    let db = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("PostgreSQL provisioning connection");
    let result = provision(&db).await;
    db.close().await;
    if let Err(error) = result {
        eprintln!("Provisioning failed: {error}");
        std::process::exit(1);
    }
    let operator = ["PLATFORM_ADMIN_EMAIL", "PLATFORM_ADMIN_PASSWORD"]
        .iter()
        .all(|key| env::var(key).is_ok_and(|v| !v.is_empty()));
    if operator {
        crate::platform::bootstrap_operator().await;
    }
    println!("Provisioning complete");
}
