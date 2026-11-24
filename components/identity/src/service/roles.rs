//! Tenant-owned roles: current grants, revisioned edits, dependency-checked removal and shared history.
use crate::domain::permissions::{SCOPES, validate_permissions};
use crate::ports::RoleLanguages;
use crate::store::roles::{current, usage};
use crate::{merchant, permit};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_app_model::declarations::identifier;
use veyndo_core::document::{lenient, view};
use veyndo_core::history_context::context;
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, State, bad, conflict};

/// The role tools of MCP and the planner.
pub const CAPABILITIES: &[(&str, &str)] = &[
    (
        "team.roles",
        "Read tenant role definitions and their revisions",
    ),
    (
        "team.role.save",
        "Create or revise a tenant role without broader delegation",
    ),
    (
        "team.role.dependencies",
        "Inspect current role membership and invitation dependencies",
    ),
    (
        "team.role.delete",
        "Remove an unused custom role with explicit approval and revision",
    ),
];
/// The input schema of a role tool.
pub fn schema(name: &str) -> Option<Value> {
    CAPABILITIES.iter().any(|(n, _)| *n == name).then(|| json!({"type":"object","properties":{"id":{"type":"string","maxLength":32},"revision":{"type":"integer","minimum":0},"approve":{"type":"boolean"},"data":{"type":"object","properties":{"name":{"type":"object","additionalProperties":{"type":"string"}},"permissions":{"type":"array","items":{"type":"string","enum":SCOPES}},"active":{"type":"boolean"}},"required":["name","permissions","active"],"additionalProperties":false}},"additionalProperties":false}))
}
/// A role operation: the role, its expected revision and the approval of a deletion.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RoleChange {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
}
/// Lists, saves, inspects or deletes a tenant role; edits are revision checked and never delegate more than the actor holds.
pub async fn invoke(
    a: &State,
    languages: &dyn RoleLanguages,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    if schema(name).is_none()
        || v.as_object().is_none_or(|o| {
            o.keys()
                .any(|k| !["id", "revision", "data", "approve"].contains(&k.as_str()))
        })
    {
        return Err(bad("Invalid role command"));
    }
    let t = merchant(h)?;
    permit(h, "team.manage")?;
    if name == "team.roles" {
        let (main_locale, locales) = languages.languages(&t).await?;
        let rows=sqlx::query("SELECT id,data,builtin,revision FROM workspace_roles WHERE tenant=$1 ORDER BY builtin DESC,id LIMIT 100").bind(t).fetch_all(&a.db).await?;
        return Ok(
            json!({"mainLocale":main_locale,"locales":locales,"elements":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"data":r.get::<Value,_>("data"),"builtin":r.get::<bool,_>("builtin"),"revision":r.get::<i64,_>("revision")})).collect::<Vec<_>>()}),
        );
    }
    let input: RoleChange = view(v);
    let id = input
        .id
        .as_deref()
        .filter(|s| identifier(s))
        .ok_or(bad("Invalid role ID"))?;
    let mut tx = a.db.begin().await?;
    veyndo_core::tenants::lock(&mut *tx, &t).await?;
    let actor = current(&mut tx, h, &t).await?;
    let row = sqlx::query(
        "SELECT revision,builtin FROM workspace_roles WHERE tenant=$1 AND id=$2 FOR UPDATE",
    )
    .bind(&t)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if name == "team.role.dependencies" {
        let mut d = usage(&mut tx, &t, id).await?;
        d["revision"] = json!(row.ok_or(bad("Role not found"))?.get::<i64, _>("revision"));
        return Ok(d);
    }
    if id == "owner" {
        return Err(bad("Owner authority cannot be changed"));
    }
    let expected = input.revision.ok_or(bad("Role revision required"))?;
    if row
        .as_ref()
        .map(|r| r.get::<i64, _>("revision"))
        .unwrap_or(0)
        != expected
    {
        return Err(conflict("Role revision changed"));
    }
    context(&mut tx, &actor, "team.roles").await?;
    if name == "team.role.delete" {
        let row = row.ok_or(bad("Role not found"))?;
        if input.approve != Some(true)
            || row.get::<bool, _>("builtin")
            || usage(&mut tx, &t, id).await?["blocked"] == true
        {
            return Err(conflict("Role is standard or referenced; pause it instead"));
        }
        sqlx::query("DELETE FROM workspace_roles WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    } else {
        let d = &v["data"];
        if d.as_object().is_none_or(|o| {
            o.len() != 3
                || o.keys()
                    .any(|k| !["name", "permissions", "active"].contains(&k.as_str()))
        }) || !d["active"].is_boolean()
            || !d["permissions"].is_array()
        {
            return Err(bad("Invalid role definition"));
        }
        validate_permissions(&actor, &d["permissions"])?;
        // Keep locale validation on the acquired connection and fence concurrent
        // settings edits until this role commits; a cached/pool read can deadlock.
        languages.check_names(&mut tx, &t, &d["name"]).await?;
        if row.is_none() {
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM workspace_roles WHERE tenant=$1")
                    .bind(&t)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= 100 {
                return Err(bad("Role limit reached"));
            }
        }
        sqlx::query("INSERT INTO workspace_roles(tenant,id,data) VALUES($1,$2,$3) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=workspace_roles.revision+1")
            .bind(&t).bind(id).bind(d).execute(&mut *tx).await?;
    }
    veyndo_core::outbox::publish_on(&mut *tx, &t, "team.role_changed", &json!({"actor":header(&actor,"x-rac-user"),"id":id,"revision":expected+1,"deleted":name=="team.role.delete"})).await?;
    tx.commit().await?;
    Ok(
        json!({"id":id,"revision":expected+1,"saved":name=="team.role.save","deleted":name=="team.role.delete"}),
    )
}
