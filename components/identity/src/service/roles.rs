//! Tenant-owned roles: current grants, revisioned edits, dependency-checked removal and shared history.
use crate::domain::permissions::{SCOPES, validate_permissions};
use crate::ports::RoleLanguages;
use crate::store::roles::{self, current, usage};
use crate::types::events::RoleChanged;
use crate::{merchant, permit};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::shop_scope::header;
use veyndo_core::store::history_context::context;
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
/// A role definition as `invoke` checks it: the shape of each field is validated before use.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RoleDefinition {
    name: Value,
    permissions: Value,
    active: Value,
}
/// The JSON fields `invoke` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InvokeInput {
    data: Value,
}

/// Lists, saves, inspects or deletes a tenant role; edits are revision checked and never delegate more than the actor holds.
pub async fn invoke(
    a: &State,
    languages: &dyn RoleLanguages,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    let args: InvokeInput = view(v);
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
        let rows = roles::list(a, &t).await?;
        return Ok(
            json!({"mainLocale":main_locale,"locales":locales,"elements":rows.iter().map(|r|json!({"id":r.id,"data":r.data,"builtin":r.builtin,"revision":r.revision})).collect::<Vec<_>>()}),
        );
    }
    let input: RoleChange = view(v);
    let id = input
        .id
        .as_deref()
        .filter(|s| identifier(s))
        .ok_or(bad("Invalid role ID"))?;
    let mut tx = a.db.begin().await?;
    veyndo_core::store::tenants::lock(&mut *tx, &t).await?;
    let actor = current(&mut tx, h, &t).await?;
    let row = roles::lock(&mut tx, &t, id).await?;
    if name == "team.role.dependencies" {
        let mut d = usage(&mut tx, &t, id).await?;
        merge(
            &mut d,
            json!({"revision":row.ok_or(bad("Role not found"))?.revision}),
        )?;
        return Ok(d);
    }
    if id == "owner" {
        return Err(bad("Owner authority cannot be changed"));
    }
    let expected = input.revision.ok_or(bad("Role revision required"))?;
    if row.as_ref().map(|r| r.revision).unwrap_or(0) != expected {
        return Err(conflict("Role revision changed"));
    }
    context(&mut tx, &actor, "team.roles").await?;
    if name == "team.role.delete" {
        let row = row.ok_or(bad("Role not found"))?;
        if input.approve != Some(true)
            || row.builtin
            || usage(&mut tx, &t, id).await?["blocked"] == true
        {
            return Err(conflict("Role is standard or referenced; pause it instead"));
        }
        roles::delete(&mut tx, &t, id).await?;
    } else {
        let d = &args.data;
        let definition: RoleDefinition = view(d);
        if d.as_object().is_none_or(|o| {
            o.len() != 3
                || o.keys()
                    .any(|k| !["name", "permissions", "active"].contains(&k.as_str()))
        }) || !definition.active.is_boolean()
            || !definition.permissions.is_array()
        {
            return Err(bad("Invalid role definition"));
        }
        validate_permissions(&actor, &definition.permissions)?;
        // Keep locale validation on the acquired connection and fence concurrent
        // settings edits until this role commits; a cached/pool read can deadlock.
        languages.check_names(&mut tx, &t, &definition.name).await?;
        if row.is_none() {
            let count = roles::count(&mut tx, &t).await?;
            if count >= 100 {
                return Err(bad("Role limit reached"));
            }
        }
        roles::save(&mut tx, &t, id, d).await?;
    }
    let changed = RoleChanged {
        actor: header(&actor, "x-rac-user"),
        id,
        revision: expected + 1,
        deleted: name == "team.role.delete",
    };
    veyndo_core::events::emit(&mut *tx, &t, &changed).await?;
    tx.commit().await?;
    Ok(
        json!({"id":id,"revision":expected+1,"saved":name=="team.role.save","deleted":name=="team.role.delete"}),
    )
}
