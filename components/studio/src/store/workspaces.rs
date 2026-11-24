//! Actor-private workspace preferences: compare-and-swap on the revision, a bounded version trail and the current membership.
use crate::domain::workspace;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, State, StatusCode, bad, conflict};

/// Opens a transaction scoped to the person and refuses anyone whose membership is no longer active.
async fn begin(a: &State, t: &str, user: &str) -> Result<Transaction<'static, Postgres>> {
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT set_config('rac.tenant',$1,true),set_config('rac.actor',$2,true)")
        .bind(t)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    if !sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND active)",
    )
    .bind(t)
    .bind(user)
    .fetch_one(&mut *tx)
    .await?
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Active membership required".into(),
        ));
    }
    Ok(tx)
}

/// The saved workspace with its recent versions, or the empty one.
pub async fn read(a: &State, t: &str, user: &str) -> Result<Value> {
    let mut tx = begin(a, t, user).await?;
    let row =
        sqlx::query("SELECT revision,data FROM studio_workspaces WHERE tenant=$1 AND user_id=$2")
            .bind(t)
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?;
    let versions=sqlx::query("SELECT revision,data,created_at::text AS created_at FROM studio_workspace_versions WHERE tenant=$1 AND user_id=$2 ORDER BY revision DESC LIMIT 20")
        .bind(t).bind(user).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        json!({"revision":row.as_ref().map(|r|r.get::<i64,_>("revision")).unwrap_or(0),
        "data":row.map(|r|r.get::<Value,_>("data")).unwrap_or_else(workspace::empty),
        "versions":versions.iter().map(|r|json!({"revision":r.get::<i64,_>("revision"),"data":r.get::<Value,_>("data"),"createdAt":r.get::<String,_>("created_at")})).collect::<Vec<_>>()}),
    )
}

/// The JSON fields `save` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveInput {
    data: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Saves the workspace at the expected revision and keeps the last twenty versions.
pub async fn save(a: &State, t: &str, user: &str, v: &Value) -> Result<Value> {
    let input: SaveInput = view(v);
    let mut tx = begin(a, t, user).await?;
    let data = workspace::validate(&input.data)?;
    let expected = input
        .revision
        .filter(|n| *n >= 0)
        .ok_or(bad("Workspace revision required"))?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,945))")
        .bind(format!("{t}:{user}"))
        .execute(&mut *tx)
        .await?;
    let current = sqlx::query_scalar::<_, i64>(
        "SELECT revision FROM studio_workspaces WHERE tenant=$1 AND user_id=$2 FOR UPDATE",
    )
    .bind(t)
    .bind(user)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(0);
    if current != expected {
        return Err(conflict(
            "Workspace changed in another window; reload before saving",
        ));
    }
    let next = current + 1;
    sqlx::query("INSERT INTO studio_workspaces(tenant,user_id,revision,data) VALUES($1,$2,$3,$4) ON CONFLICT(tenant,user_id) DO UPDATE SET revision=EXCLUDED.revision,data=EXCLUDED.data,updated_at=now()")
        .bind(t).bind(user).bind(next).bind(&data).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO studio_workspace_versions(tenant,user_id,revision,data) VALUES($1,$2,$3,$4)",
    )
    .bind(t)
    .bind(user)
    .bind(next)
    .bind(&data)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM studio_workspace_versions WHERE tenant=$1 AND user_id=$2 AND revision <= $3",
    )
    .bind(t)
    .bind(user)
    .bind(next - 20)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(json!({"revision":next,"data":data}))
}

/// Whether the shop is a private sandbox without a personal preview owner.
pub async fn is_private_sandbox(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
) -> veyndo_core::Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=$1 AND preview_owner IS NULL)",
    )
    .bind(tenant)
    .fetch_one(tx)
    .await?)
}

/// Whether the shop is a personal preview that only its owner may open.
pub async fn is_personal_preview(
    a: &veyndo_core::State,
    tenant: &str,
) -> veyndo_core::Result<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=$1 AND preview_owner IS NOT NULL)").bind(tenant).fetch_one(&a.db).await?)
}
