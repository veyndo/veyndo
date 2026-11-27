//! Workspace member visibility and immediately effective role/revocation changes.
use crate::domain::credentials::role_allowed;
use crate::store::{invitations, members, roles};
use crate::types::events::MembershipUpdated;
use crate::{merchant, permit, validate_permissions};
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict};

pub async fn members(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let tenant = merchant(&h)?;
    permit(&h, "users")?;
    let rows = members::list(&a, &tenant).await?;
    let invites = invitations::pending(&a, &tenant).await?;
    Ok(Json(
        json!({"members":rows.iter().map(|r|json!({"id":r.id,"name":r.name,"email":r.email,"role":r.role,"active":r.active,"permissions":r.permissions})).collect::<Vec<_>>(),"invitations":invites.iter().map(|r|json!({"id":r.id,"email":r.email,"role":r.role,"expires":r.expires})).collect::<Vec<_>>()}),
    ))
}
/// The JSON fields `update_member` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct UpdateMemberInput {
    #[serde(deserialize_with = "lenient")]
    role: Option<String>,
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
    permissions: Value,
}

pub async fn update_member(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(user): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: UpdateMemberInput = view(&v);
    let t = merchant(&h)?;
    permit(&h, "users")?;
    let role = input.role.as_deref().ok_or(bad("Role required"))?;
    let active = input.active.ok_or(bad("Active boolean required"))?;
    let mut tx = a.db.begin().await?;
    veyndo_core::store::tenants::lock(&mut *tx, &t).await?;
    let h = roles::current(&mut tx, &h, &t).await?;
    let actor = if header(&h, "x-rac-role") == Some("owner") {
        "owner"
    } else {
        "admin"
    };
    if !role_allowed(actor, role) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Cannot assign this role".into(),
        ));
    }
    let role_grants = roles::grants(&mut tx, &t, role).await?;
    let old = members::membership(&mut tx, &t, &user)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Member not found".into()))?;
    if old.role == "owner" {
        if actor != "owner" {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Only an owner can change an owner".into(),
            ));
        }
        if old.active && (!active || role != "owner") {
            let owners = members::active_owners(&mut tx, &t).await?;
            if owners <= 1 {
                return Err(conflict(
                    "The last active owner cannot be removed or demoted",
                ));
            }
        }
    }
    let permissions = if v.get("permissions").is_some() {
        validate_permissions(&h, &input.permissions)?
    } else {
        old.permissions.clone()
    };
    validate_permissions(
        &h,
        if permissions.is_null() {
            &role_grants
        } else {
            &permissions
        },
    )?;
    if role == "owner" && !permissions.is_null() {
        return Err(bad("Owner permissions cannot be restricted"));
    }
    members::update(&mut tx, (&t, &user), (role, active), &permissions).await?;
    let updated = MembershipUpdated {
        actor: header(&h, "x-rac-user"),
        subject: &user,
        role,
        active,
        permissions: &permissions,
        previous_role: &old.role,
        previous_active: old.active,
    };
    veyndo_core::events::emit(&mut *tx, &t, &updated).await?;
    tx.commit().await?;
    Ok(Json(json!({"updated":true})))
}
