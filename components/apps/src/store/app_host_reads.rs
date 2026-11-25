//! Reads and guarded writes the server's app host needs: connected frontends, the package listing, submission throttling and the locked checks of a native job import.
use crate::store::{job_creators, locks};
use serde_json::{Value, json};
use veyndo_core::{Error, Result, State, StatusCode, conflict};

pub use veyndo_shop::store::app_frontends::{Connection, connections};

/// Every installed package of the shop with version, manifest, state, revision and digest.
pub async fn packages(a: &State, t: &str) -> Result<Vec<Value>> {
    let rows = sqlx::query!(
        "SELECT id,version,manifest,active,revision,digest FROM app_packages WHERE tenant=$1 ORDER BY id",
        t
    )
    .fetch_all(&a.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            json!({"id":r.id,"version":r.version,"manifest":r.manifest,"active":r.active,"revision":r.revision,"digest":r.digest})
        })
        .collect())
}

/// Opens the submission transaction context: anonymous app source, the per-shop submission lock, and the submissions of the last minute for the shop and for this intake key.
pub async fn submission_window(
    tx: &mut sqlx::PgConnection,
    t: &str,
    kind: &str,
    intake_key: &str,
) -> Result<(i64, i64)> {
    sqlx::query!(
        "SELECT set_config('veyndo.source','app.submit',true) AS source,set_config('veyndo.actor','',true) AS actor"
    )
    .fetch_one(&mut *tx)
    .await?;
    locks::advisory(tx, t, 946).await?;
    let count = veyndo_core::store::outbox_reads::last_minute(tx, t, kind).await?;
    let personal =
        veyndo_core::store::outbox_reads::last_minute_for_intake(tx, t, kind, intake_key).await?;
    Ok((count, personal))
}

/// Publishes one accepted submission event.
pub async fn submitted(
    tx: &mut sqlx::PgConnection,
    t: &str,
    kind: &str,
    data: &Value,
) -> Result<()> {
    veyndo_core::store::outbox::publish_on(tx, t, kind, data).await?;
    Ok(())
}

/// The running job a native import step belongs to, under its lease and the 30-minute bound: actor, action, package digest and reviewed input.
pub async fn claimed_job(
    tx: &mut sqlx::PgConnection,
    live: &str,
    (app, id, lease): (&str, Option<&str>, Option<&str>),
) -> Result<(String, String, String, Value)> {
    locks::tenant_scope(tx, live).await?;
    let row = sqlx::query!(
        "SELECT actor,action,package_digest,input FROM app_jobs WHERE tenant=$1 AND app=$2 AND id=$3 AND status='running' AND lease=$4 AND lease_until>now() AND now()<started_at+interval '30 minutes' FOR SHARE",
        live,
        app,
        id,
        lease
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(conflict("Job is cancelled, expired or has another lease"))?;
    Ok((row.actor, row.action, row.package_digest, row.input))
}

/// The job creator's current role and rights in the live shop, locked against concurrent role changes.
pub async fn creator_grants(
    tx: &mut sqlx::PgConnection,
    live: &str,
    actor: &str,
) -> Result<(String, Value)> {
    let gone = || {
        Error(
            StatusCode::FORBIDDEN,
            "Job creator no longer has access".into(),
        )
    };
    if !job_creators::lock_member(tx, live, actor).await? {
        return Err(gone());
    }
    let grants = job_creators::membership(tx, live, actor)
        .await?
        .ok_or_else(gone)?;
    Ok((grants.role, grants.permissions))
}

/// Locks the reviewed target as an admitted private environment of the live shop.
pub async fn private_target(tx: &mut sqlx::PgConnection, target: &str, live: &str) -> Result<()> {
    if !veyndo_core::store::previews::lock_shared(tx, target, live).await? {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Job target is not an admitted private environment".into(),
        ));
    }
    Ok(())
}
