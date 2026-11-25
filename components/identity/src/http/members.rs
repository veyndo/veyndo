//! Workspace member visibility and immediately effective role/revocation changes.
use crate::domain::credentials::role_allowed;
use crate::store::roles;
use crate::{merchant, permit, validate_permissions};
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict};

pub async fn members(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let tenant = merchant(&h)?;
    permit(&h, "users")?;
    let rows=sqlx::query("SELECT u.id,u.name,u.email,m.role,m.active,m.permissions FROM memberships m JOIN merchant_users u ON u.id=m.user_id WHERE m.tenant=$1 ORDER BY u.name").bind(&tenant).fetch_all(&a.db).await?;
    let invites=sqlx::query("SELECT id,email,role,expires_at::text AS expires FROM user_invites WHERE tenant=$1 AND redeemed_at IS NULL AND expires_at>now() ORDER BY expires_at").bind(&tenant).fetch_all(&a.db).await?;
    Ok(Json(
        json!({"members":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"email":r.get::<String,_>("email"),"role":r.get::<String,_>("role"),"active":r.get::<bool,_>("active"),"permissions":r.get::<Value,_>("permissions")})).collect::<Vec<_>>(),"invitations":invites.iter().map(|r|json!({"id":r.get::<String,_>("id"),"email":r.get::<String,_>("email"),"role":r.get::<String,_>("role"),"expires":r.get::<String,_>("expires")})).collect::<Vec<_>>()}),
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
    let old = sqlx::query(
        "SELECT role,active,permissions FROM memberships WHERE tenant=$1 AND user_id=$2",
    )
    .bind(&t)
    .bind(&user)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Member not found".into()))?;
    if old.get::<String, _>("role") == "owner" {
        if actor != "owner" {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Only an owner can change an owner".into(),
            ));
        }
        if old.get::<bool, _>("active") && (!active || role != "owner") {
            let owners: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM memberships WHERE tenant=$1 AND role='owner' AND active",
            )
            .bind(&t)
            .fetch_one(&mut *tx)
            .await?;
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
        old.get("permissions")
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
    sqlx::query(
        "UPDATE memberships SET role=$1,active=$2,permissions=$5 WHERE tenant=$3 AND user_id=$4",
    )
    .bind(role)
    .bind(active)
    .bind(&t)
    .bind(&user)
    .bind(&permissions)
    .execute(&mut *tx)
    .await?;
    veyndo_core::store::outbox::publish_on(&mut *tx, &t, "membership.updated", &json!({"actor":header(&h,"x-rac-user"),"subject":user,"role":role,"active":active,"permissions":permissions,"previousRole":old.get::<String,_>("role"),"previousActive":old.get::<bool,_>("active")})).await?;
    tx.commit().await?;
    Ok(Json(json!({"updated":true})))
}
