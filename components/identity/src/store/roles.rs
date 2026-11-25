//! Tenant-owned roles: the grants of a role, the current actor, what still uses a role, and the revisioned role definitions themselves.
use crate::{SCOPES, allowed, permit};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};

pub async fn grants(conn: &mut PgConnection, tenant: &str, role: &str) -> Result<Value> {
    sqlx::query_scalar("SELECT data->'permissions' FROM workspace_roles WHERE tenant=$1 AND id=$2 AND data->>'active'='true'")
        .bind(tenant).bind(role).fetch_optional(conn).await?.ok_or(bad("Role unavailable"))
}
/// Rehydrate asynchronous/delegating actors under the tenant lifecycle lock. Owner authority is reserved.
pub async fn current(
    conn: &mut PgConnection,
    h: &RequestContext,
    tenant: &str,
) -> Result<RequestContext> {
    let mut actor = h.clone();
    if header(h, "x-rac-user") != Some("bootstrap") {
        let r=sqlx::query("SELECT role,permissions FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND active")
            .bind(tenant).bind(header(h,"x-rac-user").unwrap_or("")).fetch_optional(conn).await?
            .ok_or(Error(StatusCode::FORBIDDEN,"Membership or role is paused".into()))?;
        let permissions: Value = r.get("permissions");
        actor.principal.role = Some(r.get("role"));
        // Integration-key constraints may never be removed by a fresh membership read.
        let effective: Vec<_> = SCOPES
            .iter()
            .filter(|scope| {
                allowed(h, scope)
                    && (actor.principal.role.as_deref() == Some("owner")
                        || permissions
                            .as_array()
                            .is_some_and(|p| p.iter().any(|s| s.as_str() == Some(scope))))
            })
            .collect();
        actor.principal.permissions = Some(json!(effective).to_string());
        if header(h, "x-rac-role") != Some("owner")
            && actor.principal.role.as_deref() == Some("owner")
        {
            actor.principal.role = Some("admin".into());
        }
    }
    if permit(&actor, "team.manage").is_err() {
        return Err(conflict(
            "Team permissions changed; reload before delegating",
        ));
    }
    Ok(actor)
}
/// How many members and invitations still name a role; a used role cannot be removed.
pub async fn usage(conn: &mut PgConnection, t: &str, id: &str) -> Result<Value> {
    let r=sqlx::query("SELECT (SELECT count(*) FROM memberships WHERE tenant=$1 AND role=$2) AS members,(SELECT count(*) FROM user_invites WHERE tenant=$1 AND role=$2) AS invitations")
        .bind(t).bind(id).fetch_one(conn).await?;
    let members: i64 = r.get("members");
    let invitations: i64 = r.get("invitations");
    Ok(json!({"members":members,"invitations":invitations,"blocked":members+invitations>0}))
}

/// A role definition with its revision.
pub struct RoleRow {
    pub id: String,
    pub data: Value,
    pub builtin: bool,
    pub revision: i64,
}

/// The shop's roles, standard ones first (at most 100).
pub async fn list(a: &State, tenant: &str) -> Result<Vec<RoleRow>> {
    Ok(sqlx::query_as!(
        RoleRow,
        "SELECT id,data,builtin,revision FROM workspace_roles WHERE tenant=$1 ORDER BY builtin DESC,id LIMIT 100",
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}

/// The revision of a role and whether it is a standard role.
pub struct RoleLock {
    pub revision: i64,
    pub builtin: bool,
}

/// Locks a role for a revisioned change.
pub async fn lock(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<RoleLock>> {
    Ok(sqlx::query_as!(
        RoleLock,
        "SELECT revision,builtin FROM workspace_roles WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// Removes a role.
pub async fn delete(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM workspace_roles WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// How many roles the shop defines.
pub async fn count(conn: &mut PgConnection, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM workspace_roles WHERE tenant=$1"#,
        tenant
    )
    .fetch_one(conn)
    .await?)
}

/// Creates a role or replaces its definition and advances its revision.
pub async fn save(conn: &mut PgConnection, tenant: &str, id: &str, data: &Value) -> Result<()> {
    sqlx::query!(
        "INSERT INTO workspace_roles(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=workspace_roles.revision+1",
        tenant,
        id,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}
