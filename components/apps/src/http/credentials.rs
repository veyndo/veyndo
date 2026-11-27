//! App keys reuse the core integration-key store, current creator rights, expiry and immutable package digest.
use crate::domain::{action_admission, approval};
use crate::store::credentials;
use crate::store::packages::package;
use crate::types::events::CallbackApproved;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::admission::callback_scope;
use veyndo_core::document::{lenient, view};
use veyndo_core::store::environments::parent;
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
    let rows = credentials::list(&a, &t, &id).await?;
    Ok(Json(
        json!({"canManage":permit(&h,"team.manage").is_ok() && h.principal.user.as_deref().is_some_and(|u|u!="bootstrap") && parent(&a,&t).await?.is_none(),"keys":rows,"digest":approval::canonical_digest(&m),"permissions":m.permissions.iter().filter(|p|callback_scope(p).is_some()).collect::<Vec<_>>()}),
    ))
}
/// An app callback key request: the approved package digest, the permissions and the expiry.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct KeyRequest {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    digest: Option<String>,
    permissions: Value,
    #[serde(deserialize_with = "lenient")]
    expires_in_days: Option<i64>,
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
    let input: KeyRequest = view(&v);
    if input.approve != Some(true) || input.digest.as_deref() != Some(digest.as_str()) {
        return Err(conflict(
            "Approve the current app package digest and its requested callback permissions",
        ));
    }
    let scopes = veyndo_app_model::admission::requested_scopes(
        &action_admission::Authority(&h),
        &m.permissions,
        &input.permissions,
    )
    .map_err(action_admission::error)?;
    let requested = &input.permissions;
    let days = input
        .expires_in_days
        .filter(|n| (1..=90).contains(n))
        .ok_or(bad("Expiry must be 1..90 days"))?;
    let mut tx = a.db.begin().await?;
    veyndo_core::store::tenants::lock(&mut *tx, &t).await?;
    let count = credentials::active_count(&mut tx, &t).await?;
    if count >= 32 {
        return Err(bad("Maximum 32 active integration keys"));
    }
    let key = format!("rac_{}{}", uid(), uid());
    let key_id = uid();
    credentials::issue(
        &mut tx,
        credentials::NewKey {
            id: &key_id,
            tenant: &t,
            user,
            digest: &hash(&key),
            name: &format!("App {id}"),
            scopes: &json!(scopes),
            app: &id,
            package_digest: &digest,
            days: days as i32,
            app_permissions: &json!(requested),
        },
    )
    .await?;
    let approved = CallbackApproved {
        app: &id,
        digest: &digest,
        // `requested_scopes` admitted the list, so it is an array of permission names.
        permissions: requested
            .as_array()
            .map(|names| names.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default(),
        actor: user,
        key_id: &key_id,
    };
    veyndo_core::events::emit(&mut *tx, &t, &approved).await?;
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
    credentials::revoke(&a, &t, &id, &key).await?;
    Ok(Json(json!({"revoked":true})))
}
