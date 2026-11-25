//! Workspace memberships: the member list, the active grant of a person, role/activity changes and owner counting. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// A member of the shop with their account details.
pub struct MemberRow {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub active: bool,
    pub permissions: Value,
}

/// Every member of the shop, by name.
pub async fn list(a: &State, tenant: &str) -> Result<Vec<MemberRow>> {
    Ok(sqlx::query_as!(
        MemberRow,
        "SELECT u.id,u.name,u.email,m.role,m.active,m.permissions FROM memberships m JOIN merchant_users u ON u.id=m.user_id WHERE m.tenant=$1 ORDER BY u.name",
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}

/// The role and explicit permissions of an active membership grant.
pub struct Grant {
    pub role: String,
    pub permissions: Value,
}

/// The active grant of the user in the shop, if any.
pub async fn active_grant(
    conn: &mut sqlx::PgConnection,
    tenant: &str,
    user: &str,
) -> Result<Option<Grant>> {
    Ok(sqlx::query_as!(
        Grant,
        r#"SELECT role AS "role!",permissions AS "permissions!" FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND active"#,
        tenant,
        user
    )
    .fetch_optional(conn)
    .await?)
}

/// A membership as a role change reads it before writing.
pub struct Membership {
    pub role: String,
    pub active: bool,
    pub permissions: Value,
}

/// The membership of the user in the shop, active or not.
pub async fn membership(
    tx: &mut sqlx::PgConnection,
    tenant: &str,
    user: &str,
) -> Result<Option<Membership>> {
    Ok(sqlx::query_as!(
        Membership,
        "SELECT role,active,permissions FROM memberships WHERE tenant=$1 AND user_id=$2",
        tenant,
        user
    )
    .fetch_optional(tx)
    .await?)
}

/// How many active owners the shop has.
pub async fn active_owners(tx: &mut sqlx::PgConnection, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM memberships WHERE tenant=$1 AND role='owner' AND active"#,
        tenant
    )
    .fetch_one(tx)
    .await?)
}

/// Sets the role, activity and explicit permissions of a membership.
pub async fn update(
    tx: &mut sqlx::PgConnection,
    (tenant, user): (&str, &str),
    (role, active): (&str, bool),
    permissions: &Value,
) -> Result<()> {
    sqlx::query!(
        "UPDATE memberships SET role=$1,active=$2,permissions=$5 WHERE tenant=$3 AND user_id=$4",
        role,
        active,
        tenant,
        user,
        permissions
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Adds a membership; an existing one keeps its role and activity.
pub async fn add(tx: &mut sqlx::PgConnection, user: &str, tenant: &str, role: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO memberships(user_id,tenant,role) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
        user,
        tenant,
        role
    )
    .execute(tx)
    .await?;
    Ok(())
}
