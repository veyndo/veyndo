//! The merchant sessions and memberships a channel preview is bound to: whether a session is live, and the role and rights of the live session's active membership in a shop. The caller runs both in the system scope. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// Whether `digest` is a live session of `user`.
pub async fn alive(a: &State, digest: &str, user: Option<&str>) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM user_sessions WHERE digest=$1 AND user_id=$2 AND expires_at>now()) AS "alive!""#,
        digest,
        user
    )
    .fetch_one(&a.db)
    .await?)
}

/// The role and rights of an active membership.
pub struct Membership {
    pub role: String,
    pub permissions: Value,
}

/// The active membership in `tenant` of the user behind the live session `digest`.
pub async fn member(a: &State, digest: &str, tenant: &str) -> Result<Option<Membership>> {
    Ok(sqlx::query_as!(Membership, "SELECT m.role,m.permissions FROM user_sessions s JOIN memberships m ON m.user_id=s.user_id WHERE s.digest=$1 AND m.tenant=$2 AND s.expires_at>now() AND m.active",
        digest, tenant)
    .fetch_optional(&a.db)
    .await?)
}
