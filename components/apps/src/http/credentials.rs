//! App keys reuse the core integration-key store, current creator rights, expiry and immutable package digest.
use crate::domain::{action_admission, approval};
use crate::store::packages::package;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use veyndo_app_model::admission::callback_scope;
use veyndo_core::shop_scope::parent;
use veyndo_core::{RequestContext, Result, State, bad, conflict, hash, uid};
use veyndo_identity::{merchant, permit};

/// The callback keys of an app; never a key value.
pub async fn list(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let m = package(&a, &t, &id, false).await?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'permissions',app_permissions,'createdAt',created_at,'expiresAt',expires_at,'creator',user_id,'packageDigest',package_digest) FROM integration_keys WHERE tenant=$1 AND app_id=$2 ORDER BY created_at DESC LIMIT 100").bind(&t).bind(id).fetch_all(&a.db).await?;
    Ok(Json(
        json!({"canManage":permit(&h,"team.manage").is_ok() && h.principal.user.as_deref().is_some_and(|u|u!="bootstrap") && parent(&a,&t).await?.is_none(),"keys":rows,"digest":approval::canonical_digest(&m),"permissions":m.permissions.iter().filter(|p|callback_scope(p).is_some()).collect::<Vec<_>>()}),
    ))
}
/// Issues one callback key for the approved digest and permissions; the key is shown once.
pub async fn create(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "team.manage")?;
    let user = h
        .principal
        .user
        .as_deref()
        .filter(|u| *u != "bootstrap")
        .ok_or(bad("Personal account required"))?;
    if parent(&a, &t).await?.is_some() {
        return Err(bad("App callback keys require a live workspace"));
    }
    let m = package(&a, &t, &id, true).await?;
    let digest = approval::canonical_digest(&m);
    if v["approve"] != true || v["digest"] != digest {
        return Err(conflict(
            "Approve the current app package digest and its requested callback permissions",
        ));
    }
    let scopes = veyndo_app_model::admission::requested_scopes(
        &action_admission::Authority(&h),
        &m.permissions,
        &v["permissions"],
    )
    .map_err(action_admission::error)?;
    let requested = &v["permissions"];
    let days = v["expiresInDays"]
        .as_i64()
        .filter(|n| (1..=90).contains(n))
        .ok_or(bad("Expiry must be 1..90 days"))?;
    let mut tx = a.db.begin().await?;
    veyndo_core::tenants::lock(&mut *tx, &t).await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration_keys WHERE tenant=$1 AND expires_at>now()",
    )
    .bind(&t)
    .fetch_one(&mut *tx)
    .await?;
    if count >= 32 {
        return Err(bad("Maximum 32 active integration keys"));
    }
    let key = format!("rac_{}{}", uid(), uid());
    let key_id = uid();
    sqlx::query("INSERT INTO integration_keys(id,tenant,user_id,digest,name,permissions,app_id,package_digest,app_permissions,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$10,now()+$9*interval '1 day')").bind(&key_id).bind(&t).bind(user).bind(hash(&key)).bind(format!("App {id}" )).bind(json!(scopes)).bind(&id).bind(&digest).bind(days as i32).bind(json!(requested)).execute(&mut *tx).await?;
    veyndo_core::outbox::publish_on(
        &mut *tx,
        &t,
        "app.callback_approved",
        &json!({"app":id,"digest":digest,"permissions":requested,"actor":user,"keyId":key_id}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"id":key_id,"key":key,"permissions":requested,"coreScopes":scopes,"digest":digest,"expiresInDays":days}),
    ))
}
/// Revokes one callback key.
pub async fn revoke(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((id, key)): Path<(String, String)>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "team.manage")?;
    sqlx::query("DELETE FROM integration_keys WHERE tenant=$1 AND app_id=$2 AND id=$3")
        .bind(t)
        .bind(id)
        .bind(key)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"revoked":true})))
}
