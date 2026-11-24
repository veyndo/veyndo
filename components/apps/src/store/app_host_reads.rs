//! Reads and guarded writes the server's app host needs: connected frontends, the package listing, submission throttling and the locked checks of a native job import.
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Error, Result, State, StatusCode, conflict};

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
    let rows=sqlx::query("SELECT alias,channel,experience_alias,revision,app_id FROM hosted_frontends WHERE tenant=$1 ORDER BY alias")
        .bind(t).fetch_all(&a.db).await?;
    Ok(rows
        .iter()
        .map(|r| Connection {
            alias: r.get("alias"),
            channel: r.get("channel"),
            experience: r.get("experience_alias"),
            revision: r.get("revision"),
            app: r.get("app_id"),
        })
        .collect())
}

/// Every installed package of the shop with version, manifest, state, revision and digest.
pub async fn packages(a: &State, t: &str) -> Result<Vec<Value>> {
    let rows=sqlx::query("SELECT id,version,manifest,active,revision,digest FROM app_packages WHERE tenant=$1 ORDER BY id").bind(t).fetch_all(&a.db).await?;
    Ok(rows
        .iter()
        .map(|r| {
            json!({"id":r.get::<String,_>("id"),"version":r.get::<String,_>("version"),"manifest":r.get::<Value,_>("manifest"),"active":r.get::<bool,_>("active"),"revision":r.get::<i64,_>("revision"),"digest":r.get::<String,_>("digest")})
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
    sqlx::query(
        "SELECT set_config('veyndo.source','app.submit',true),set_config('veyndo.actor','',true)",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,946))")
        .bind(t)
        .execute(&mut *tx)
        .await?;
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM outbox WHERE tenant=$1 AND kind=$2 AND created_at>now()-interval '1 minute'").bind(t).bind(kind).fetch_one(&mut *tx).await?;
    let personal:i64=sqlx::query_scalar("SELECT count(*) FROM outbox WHERE tenant=$1 AND kind=$2 AND data->>'intakeKey'=$3 AND created_at>now()-interval '1 minute'").bind(t).bind(kind).bind(intake_key).fetch_one(&mut *tx).await?;
    Ok((count, personal))
}

/// Publishes one accepted submission event.
pub async fn submitted(
    tx: &mut sqlx::PgConnection,
    t: &str,
    kind: &str,
    data: &Value,
) -> Result<()> {
    veyndo_core::outbox::publish_on(tx, t, kind, data).await?;
    Ok(())
}

/// The running job a native import step belongs to, under its lease and the 30-minute bound: actor, action, package digest and reviewed input.
pub async fn claimed_job(
    tx: &mut sqlx::PgConnection,
    live: &str,
    (app, id, lease): (&str, Option<&str>, Option<&str>),
) -> Result<(String, String, String, Value)> {
    sqlx::query("SELECT set_config('rac.tenant',$1,true)")
        .bind(live)
        .execute(&mut *tx)
        .await?;
    let row=sqlx::query("SELECT actor,action,package_digest,input FROM app_jobs WHERE tenant=$1 AND app=$2 AND id=$3 AND status='running' AND lease=$4 AND lease_until>now() AND now()<started_at+interval '30 minutes' FOR SHARE").bind(live).bind(app).bind(id).bind(lease).fetch_optional(&mut *tx).await?.ok_or(conflict("Job is cancelled, expired or has another lease"))?;
    Ok((
        row.get("actor"),
        row.get("action"),
        row.get("package_digest"),
        row.get("input"),
    ))
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
    sqlx::query("SELECT m.user_id FROM memberships m JOIN workspace_roles r ON r.tenant=m.tenant AND r.id=m.role WHERE m.tenant=$1 AND m.user_id=$2 FOR SHARE OF m,r").bind(live).bind(actor).fetch_optional(&mut *tx).await?.ok_or_else(gone)?;
    let grants = sqlx::query(
        "SELECT role,permissions FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND active",
    )
    .bind(live)
    .bind(actor)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(gone)?;
    Ok((grants.get("role"), grants.get("permissions")))
}

/// Locks the reviewed target as an admitted private environment of the live shop.
pub async fn private_target(tx: &mut sqlx::PgConnection, target: &str, live: &str) -> Result<()> {
    sqlx::query("SELECT tenant FROM shop_environments WHERE tenant=$1 AND live_tenant=$2 AND preview_owner IS NULL FOR SHARE").bind(target).bind(live).fetch_optional(tx).await?.ok_or(Error(StatusCode::FORBIDDEN,"Job target is not an admitted private environment".into()))?;
    Ok(())
}
