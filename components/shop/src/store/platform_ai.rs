//! The operator inference configuration in storage: the singleton row, its revision-locked update and the audit entry. Columns checked at compile time.
use serde_json::Value;
use sqlx::{PgConnection, PgExecutor};
use veyndo_core::Result;

/// The stored configuration with its sealed keys.
pub struct Stored {
    pub revision: i64,
    pub data: Value,
    pub secrets: Value,
    pub updated: String,
}

/// The stored configuration.
pub async fn read(e: impl PgExecutor<'_>) -> Result<Stored> {
    Ok(sqlx::query_as!(
        Stored,
        r#"SELECT revision,data,secrets,updated_at::text AS "updated!" FROM platform_ai WHERE id=true"#
    )
    .fetch_one(e)
    .await?)
}

/// The revision and sealed keys, row-locked for an update.
pub struct Locked {
    pub revision: i64,
    pub secrets: Value,
}

/// Locks the configuration row.
pub async fn lock(e: impl PgExecutor<'_>) -> Result<Locked> {
    Ok(sqlx::query_as!(
        Locked,
        "SELECT revision,secrets FROM platform_ai WHERE id=true FOR UPDATE"
    )
    .fetch_one(e)
    .await?)
}

/// Saves the configuration and appends its audit entry (no secrets).
pub async fn save(
    tx: &mut PgConnection,
    data: &Value,
    secrets: &Value,
    actor: &str,
    audit: &Value,
) -> Result<()> {
    sqlx::query!("UPDATE platform_ai SET revision=revision+1,data=$1,secrets=$2,updated_by=$3,updated_at=now() WHERE id=true",
        data, secrets, actor).execute(&mut *tx).await?;
    sqlx::query!(
        "INSERT INTO platform_audit(actor,action,data) VALUES($1,'ai.updated',$2)",
        actor,
        audit
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}
