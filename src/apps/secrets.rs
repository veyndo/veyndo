//! Digest-bound per-shop app secret rotation reuses platform authenticated encryption; no plaintext read endpoint.
use super::*;
use veyndo_app_platform::domain::secret_kinds::{KINDS, applicable};
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/secrets",
            &[("GET", "apps.manage")],
            get(list),
        )
        .secure_route(
            "/api/apps/{id}/secrets/{kind}",
            &[("PUT", "apps.manage")],
            axum::routing::put(rotate),
        )
}
async fn list(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    let m = package(&a, &t, &id, false).await?;
    let mut tx = a.db.begin().await?;
    let secrets = veyndo_app_storage::secrets::list(
        &mut tx,
        veyndo_app_storage::Scope {
            tenant: &t,
            app: &id,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"digest":approval::canonical_digest(&m),"kinds":KINDS.iter().filter(|k|applicable(&m,k)).collect::<Vec<_>>(),"canManage":auth::permit(&h,"team.manage").is_ok() && h.principal.user.as_deref().is_some_and(|u|u!="bootstrap") && staging::parent(&a,&t).await?.is_none(),"secrets":secrets,"encryptionReady":veyndo::inference::settings::encryption_ready()}),
    ))
}
async fn rotate(
    State(a): State<App>,
    h: RequestContext,
    Path((id, kind)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "team.manage")?;
    let actor = h
        .principal
        .user
        .as_deref()
        .filter(|u| *u != "bootstrap")
        .ok_or(bad("Personal account required"))?;
    if !KINDS.contains(&kind.as_str()) || staging::parent(&a, &t).await?.is_some() {
        return Err(bad("Secret kind or live workspace is invalid"));
    }
    let m = package(&a, &t, &id, true).await?;
    if !applicable(&m, &kind) {
        return Err(bad("Secret kind is not declared by this app"));
    }
    let digest = approval::canonical_digest(&m);
    let revision = v["revision"]
        .as_i64()
        .filter(|v| *v >= 0)
        .ok_or(bad("Secret revision required"))?;
    if v["approve"] != true || v["digest"] != digest {
        return Err(conflict(
            "Approve the current package digest before rotating its secret",
        ));
    }
    let secret = v["secret"]
        .as_str()
        .filter(|s| (32..=4096).contains(&s.len()) && !s.chars().any(char::is_control))
        .ok_or(bad(
            "Secret must be 32..4096 characters without control characters",
        ))?;
    let revision = store(
        &a,
        &t,
        &id,
        SecretWrite {
            kind: &kind,
            digest: &digest,
            revision,
            actor,
            secret,
        },
    )
    .await?;
    Ok(Json(json!({"rotated":true,"revision":revision})))
}
pub(super) use veyndo_app_platform::store::secrets::{SecretWrite, resolve, resolve_tx, store};
