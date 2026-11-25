//! The singleton operator inference configuration (`platform_ai`) with its sealed provider secrets, as model calls inherit it.
use serde_json::Value;
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

/// The platform AI configuration and its sealed provider secrets.
pub struct PlatformAi {
    pub data: Value,
    pub secrets: Value,
}

/// Reads the single platform AI row; the caller runs it in system scope.
pub async fn read<'e>(db: impl Executor<'e, Database = Postgres>) -> Result<PlatformAi> {
    Ok(sqlx::query_as!(
        PlatformAi,
        "SELECT data,secrets FROM platform_ai WHERE id=true"
    )
    .fetch_one(db)
    .await?)
}
