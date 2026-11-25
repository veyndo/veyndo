//! The current membership of a member who started long app work, share-locked so its rights cannot change mid-callback.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// The role and rights of an active member.
pub struct Membership {
    pub role: String,
    pub permissions: Value,
}

/// Share-locks the member's membership and role rows; false when the member left the shop.
pub async fn lock_member(conn: &mut PgConnection, t: &str, user: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT m.user_id FROM memberships m JOIN workspace_roles r ON r.tenant=m.tenant AND r.id=m.role WHERE m.tenant=$1 AND m.user_id=$2 FOR SHARE OF m,r",
        t,
        user
    )
    .fetch_optional(conn)
    .await?
    .is_some())
}

/// The current role and rights of an active member, from the canonical grant projection.
pub async fn membership(
    conn: &mut PgConnection,
    t: &str,
    user: &str,
) -> Result<Option<Membership>> {
    Ok(sqlx::query_as!(
        Membership,
        r#"SELECT role AS "role!",permissions AS "permissions!" FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND active"#,
        t,
        user
    )
    .fetch_optional(conn)
    .await?)
}
