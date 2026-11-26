//! App samples for the merchant workspace: active apps with failed jobs and the activity check of workflow source apps.
use veyndo_core::{Result, State};

/// Whether the app is installed and active.
pub async fn app_active(c: &mut sqlx::PgConnection, t: &str, app: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM app_packages WHERE tenant=$1 AND id=$2 AND active) AS "exists!""#, t, app)
    .fetch_one(&mut *c)
    .await?)
}

/// Up to three active apps with a failed job.
pub async fn failed_job_apps(a: &State, t: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT DISTINCT j.app FROM app_jobs j JOIN app_packages p ON p.tenant=j.tenant AND p.id=j.app WHERE j.tenant=$1 AND j.status='failed' AND p.active ORDER BY j.app LIMIT 3", t).fetch_all(&a.db).await?)
}
