//! The operator inference configuration every model call inherits, read with its sealed provider secrets.
use crate::scoped_pool::ScopedPool;
use serde_json::Value;

/// The platform AI configuration and its sealed provider secrets.
pub struct PlatformAi {
    pub data: Value,
    pub secrets: Value,
}

/// Reads the single platform AI row; the caller runs it in system scope.
pub async fn platform(db: &ScopedPool) -> sqlx::Result<PlatformAi> {
    sqlx::query_as!(
        PlatformAi,
        "SELECT data,secrets FROM platform_ai WHERE id=true"
    )
    .fetch_one(db)
    .await
}
