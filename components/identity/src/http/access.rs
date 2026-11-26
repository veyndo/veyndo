//! What a signed-in person may do in this workspace, their own sessions, and revocation of sessions and invitations.
use crate::store::{invitations, sessions};
use crate::{SCOPES, allowed, merchant, permit};
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
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
    let rows = sessions::active_sessions(&a, user).await?;
    let current = header(&h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(hash);
    Ok(Json(
        json!({"sessions":rows.iter().map(|r|json!({"id":r.digest,"current":current.as_ref()==Some(&r.digest),"createdAt":r.created,"expiresAt":r.expires})).collect::<Vec<_>>()}),
    ))
}
pub async fn revoke_session(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    merchant(&h)?;
    sessions::revoke(&a, header(&h, "x-rac-user").unwrap(), &id).await?;
    Ok(Json(json!({"revoked":true})))
}
pub async fn revoke_invite(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "users")?;
    invitations::revoke(&a, &t, &id).await?;
    Ok(Json(json!({"revoked":true})))
}
