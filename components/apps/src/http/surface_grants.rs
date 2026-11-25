//! Short-lived UI grants bind a package, surface, actor and concrete editor object on the server.
use crate::domain::action_admission::authorized;
use crate::domain::action_policy::readonly;
use crate::domain::approval;
use crate::domain::surface_grant::{actor, bind_input};
use crate::ports::Platform;
use crate::service::gateway;
use crate::store::locks;
use crate::store::packages::package;
use crate::store::surface_grants::{self, context, load};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash, uid};
use veyndo_identity::{merchant, permit};
use veyndo_kernel::verified_kernel;

/// The JSON fields `create` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CreateInput {
    #[serde(deserialize_with = "lenient")]
    read_only: Option<bool>,
}

/// Issues a five-minute grant for one surface, bound to the caller and the objects it names.
pub async fn create(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path((id, surface)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: CreateInput = view(&v);
    let t = tenant(&h)?;
    let m = package(&a, &t, &id, true).await?;
    let s = m
        .surfaces
        .iter()
        .find(|s| s.id == surface)
        .ok_or(bad("Unknown app surface"))?;
    if s.location.starts_with("admin.") {
        merchant(&h)?;
    }
    if let Some(p) = &s.permission {
        permit(&h, p)?;
    }
    let ctx = v.get("context").cloned().unwrap_or(json!({}));
    context(&a, &h, &ctx).await?;
    let actions = s
        .actions
        .iter()
        .filter(|name| {
            m.actions.iter().any(|act| {
                act.name == **name
                    && authorized(&h, act)
                    && (input.read_only != Some(true) || readonly(act))
            })
        })
        .collect::<Vec<_>>();
    let grant = format!("{}{}", uid(), uid());
    let subject = actor(&h);
    let mut tx = a.db.begin().await?;
    locks::advisory(&mut tx, &format!("{t}:{id}:{subject}"), 812).await?;
    surface_grants::expire(&mut tx, &t, &id).await?;
    let count = surface_grants::actor_count(&mut tx, &t, &id, &subject).await?;
    if count >= 100 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many active app surfaces; retry after expiry".into(),
        ));
    }
    surface_grants::grant(
        &mut tx,
        surface_grants::NewGrant {
            tenant: &t,
            token_hash: &hash(&grant),
            app: &id,
            surface: &surface,
            actor: &subject,
            package_digest: &approval::canonical_digest(&m),
            actions: &json!(actions),
            context: &ctx,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"token":grant,"expiresIn":300,"actions":actions}),
    ))
}
/// The JSON fields `invoke` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InvokeInput {
    input: Value,
    #[serde(deserialize_with = "lenient")]
    request_key: Option<String>,
}

/// Runs an action inside a surface grant, on the object the surface was opened for.
pub async fn invoke(
    Shared(a): Shared<Platform>,
    mut h: RequestContext,
    Path((id, surface, name)): Path<(String, String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let args: InvokeInput = view(&v);
    let (m, s, ctx, actions) = load(&a, &h, &id, &surface, &v).await?;
    if !verified_kernel::app_surface_admissible(
        true,
        actions.as_array().is_some_and(|a| a.contains(&json!(name))),
    ) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Action is outside the current surface grant".into(),
        ));
    }
    bind_input(&m, &s, &ctx, &name, &args.input)?;
    if let Some(key) = args
        .request_key
        .as_deref()
        .filter(|s| !s.is_empty() && s.len() <= 100)
    {
        h.insert(
            "idempotency-key",
            key.parse().map_err(|_| bad("Invalid request key"))?,
        );
    }
    Ok(Json(
        gateway::invoke(&a, &h, &id, &name, &args.input, None).await?,
    ))
}
