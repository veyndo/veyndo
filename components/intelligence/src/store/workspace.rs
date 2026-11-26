//! Agent run samples for the merchant workspace: the actor's failed or uncertain runs within their scopes.
use serde_json::Value;
use veyndo_core::{Result, State};

/// The actor's three most recently updated failed or uncertain runs within the given scopes.
pub async fn unsettled_runs(
    a: &State,
    t: &str,
    actor: &str,
    scopes: &Value,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT id FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_state IN ('failed','uncertain') AND run_scopes <@ $3 ORDER BY run_updated DESC LIMIT 3", t, actor, scopes).fetch_all(&a.db).await?)
}

/// Replaces the storefront experience at its expected revision; returns the rows changed.
pub async fn replace_experience(
    c: &mut sqlx::PgConnection,
    t: &str,
    data: Value,
    expected: Option<i64>,
) -> Result<u64> {
    Ok(sqlx::query!(
        "UPDATE experiences SET data=$1,revision=revision+1 WHERE tenant=$2 AND revision=$3",
        data,
        t,
        expected
    )
    .execute(&mut *c)
    .await?
    .rows_affected())
}
