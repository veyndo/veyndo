//! What a signed-in person may do in this workspace, their own sessions, and revocation of sessions and invitations.
use crate::{SCOPES, allowed, merchant, permit};
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, hash};

pub async fn access(h: RequestContext) -> Result<Json<Value>> {
    merchant(&h)?;
    Ok(Json(
        json!({"permissions":SCOPES.iter().filter(|s|allowed(&h,s)).collect::<Vec<_>>(),"available":SCOPES,"personalSession":h.principal.personal_session}),
    ))
}
pub async fn sessions(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    merchant(&h)?;
    let user = header(&h, "x-rac-user").unwrap();
    let rows=sqlx::query("SELECT digest,created_at::text AS created,expires_at::text AS expires FROM user_sessions WHERE user_id=$1 AND expires_at>now() ORDER BY created_at DESC LIMIT 100").bind(user).fetch_all(&a.db).await?;
    let current = header(&h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(hash);
    Ok(Json(
        json!({"sessions":rows.iter().map(|r|{let id:String=r.get("digest");json!({"id":id,"current":current.as_ref()==Some(&id),"createdAt":r.get::<String,_>("created"),"expiresAt":r.get::<String,_>("expires")})}).collect::<Vec<_>>()}),
    ))
}
pub async fn revoke_session(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    merchant(&h)?;
    sqlx::query("DELETE FROM user_sessions WHERE user_id=$1 AND digest=$2")
        .bind(header(&h, "x-rac-user").unwrap())
        .bind(id)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"revoked":true})))
}
pub async fn revoke_invite(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "users")?;
    sqlx::query("UPDATE user_invites SET redeemed_at=now() WHERE tenant=$1 AND id=$2 AND redeemed_at IS NULL").bind(t).bind(id).execute(&a.db).await?;
    Ok(Json(json!({"revoked":true})))
}
