//! Stored package contracts: written with the package on the installation transaction, immutable per published version, trusted only when they name the row's version.
use crate::domain::package_contract::{self, Contract};
use crate::domain::port_wiring::{self, Installed};
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad, conflict};

/// A stored contract, if it still describes the stored version.
fn current(contract: Option<Value>, version: &str) -> Option<Contract> {
    contract
        .and_then(|c| serde_json::from_value::<Contract>(c).ok())
        .filter(|c| c.version == version)
}
/// The contracts of every active package of a shop, by app ID.
pub async fn active<'e>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    t: &str,
) -> Result<Vec<(String, Contract)>> {
    Ok(sqlx::query!(
        "SELECT id,version,contract FROM app_packages WHERE tenant=$1 AND active AND contract IS NOT NULL ORDER BY id",
        t
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .filter_map(|r| current(r.contract, &r.version).map(|c| (r.id, c)))
    .collect())
}
/// The current contract of one installed package.
pub async fn of_app<'e>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    t: &str,
    app: &str,
) -> Result<Option<Contract>> {
    let row = sqlx::query!(
        "SELECT version,contract FROM app_packages WHERE tenant=$1 AND id=$2",
        t,
        app
    )
    .fetch_optional(db)
    .await?;
    Ok(row.and_then(|r| current(r.contract, &r.version)))
}
/// One published version of a package with its contract.
pub struct Version {
    pub version: String,
    pub digest: String,
    pub manifest: Value,
    pub contract: Option<Contract>,
    pub installed_at: String,
}
/// The published versions of a package, newest installation first.
pub async fn versions(conn: &mut PgConnection, t: &str, app: &str) -> Result<Vec<Version>> {
    Ok(sqlx::query!(
        r#"SELECT version,digest,manifest,contract,installed_at::text AS "installed_at!" FROM app_versions WHERE tenant=$1 AND app=$2 ORDER BY installed_at DESC, version DESC LIMIT 50"#,
        t,
        app
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .map(|r| Version {
        contract: current(r.contract, &r.version),
        version: r.version,
        digest: r.digest,
        manifest: r.manifest,
        installed_at: r.installed_at,
    })
    .collect())
}
/// Admits and stores the contract of the package the caller just installed, on the same transaction
/// and under its tenant installation lock: ports must stay wired for every active app, and a
/// published version keeps the contract it was first published with.
pub async fn install(
    tx: &mut PgConnection,
    t: &str,
    m: &Manifest,
    contract: Option<&Contract>,
    publishers: &Value,
    versions: bool,
) -> Result<()> {
    // A package without a contract keeps the conservative tool defaults and declares no ports.
    if let Some(c) = contract {
        package_contract::validate(m, c, publishers)?;
    }
    let others = active(&mut *tx, t).await?;
    let installed: Vec<Installed<'_>> = others
        .iter()
        .filter(|(app, _)| app != &m.id)
        .map(|(app, contract)| Installed { app, contract })
        .collect();
    let empty = Contract::default();
    port_wiring::admit(&m.id, contract.unwrap_or(&empty), &installed)?;
    let value = contract.map(|c| serde_json::to_value(c).expect("Serializable contract"));
    if versions {
        let stored = sqlx::query_scalar!(
            "SELECT contract FROM app_versions WHERE tenant=$1 AND app=$2 AND version=$3",
            t,
            &m.id,
            &m.version
        )
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        if let Some(stored) = stored
            && Some(&stored) != value.as_ref()
        {
            return Err(conflict("Published app version is immutable"));
        }
        sqlx::query!(
            "UPDATE app_versions SET contract=$4 WHERE tenant=$1 AND app=$2 AND version=$3 AND contract IS NULL",
            t,
            &m.id,
            &m.version,
            value.clone() as Option<Value>
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!(
        "UPDATE app_packages SET contract=$3 WHERE tenant=$1 AND id=$2",
        t,
        &m.id,
        value as Option<Value>
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}
/// Refuses deactivating a package that another active app still consumes a port from.
pub async fn admit_deactivation(tx: &mut PgConnection, t: &str, app: &str) -> Result<()> {
    let all = active(&mut *tx, t).await?;
    let installed: Vec<Installed<'_>> = all
        .iter()
        .map(|(app, contract)| Installed { app, contract })
        .collect();
    port_wiring::admit_removal(app, &installed)
}
/// Refuses activating a package whose consumed ports no active app provides.
pub async fn admit_activation(tx: &mut PgConnection, t: &str, app: &str) -> Result<()> {
    let Some(contract) = of_app(&mut *tx, t, app).await? else {
        return Ok(());
    };
    let all = active(&mut *tx, t).await?;
    let installed: Vec<Installed<'_>> = all
        .iter()
        .map(|(app, contract)| Installed { app, contract })
        .collect();
    port_wiring::admit(app, &contract, &installed)
}
/// The stored manifest and contract of one published version, for an explicit rollback.
pub async fn version(
    conn: &mut PgConnection,
    t: &str,
    app: &str,
    version: &str,
) -> Result<(Manifest, Option<Contract>)> {
    let row = sqlx::query!(
        "SELECT manifest,contract FROM app_versions WHERE tenant=$1 AND app=$2 AND version=$3",
        t,
        app,
        version
    )
    .fetch_optional(conn)
    .await?
    .ok_or(bad("This version was never installed in this shop"))?;
    let manifest: Manifest =
        serde_json::from_value(row.manifest).map_err(|_| bad("Invalid stored package version"))?;
    Ok((manifest, current(row.contract, version)))
}
