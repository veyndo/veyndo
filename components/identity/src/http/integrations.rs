//! Explicit-lifetime API/MCP keys are bounded to one workspace and intersect their creator's current membership.
use crate::domain::permissions::planning_scope;
use crate::store::integrations::{self, NewKey};
use crate::types::envelopes::Integration;
use crate::{merchant, permit, validate_permissions};
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash, uid};
use veyndo_kernel::verified_kernel;

pub async fn integration_list(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "users")?;
    let rows = integrations::list(&a, &t).await?;
    Ok(Json(
        json!({"elements":rows.iter().map(|r|json!({"id":r.id,"name":r.name,"userId":r.user_id,"appId":r.app_id,"agentBudget":r.agent_budget,"agentUsage":{"tokensAllocated":r.agent_tokens_allocated,"costMicrosAllocated":r.agent_cost_allocated},"permissions":r.permissions,"createdAt":r.created,"expiresAt":r.expires})).collect::<Vec<_>>()}),
    ))
}
pub async fn integration_create(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Json(input): Json<Integration>,
) -> Result<Json<Value>> {
    if let Some(budget) = &input.agent_budget {
        budget.validate().map_err(bad)?;
        if h.access
            .as_ref()
            .is_some_and(|scope| scope.parent.is_some())
        {
            return Err(bad(
                "Create agent credentials in the live shop, not in a sandbox",
            ));
        }
        if !h.principal.personal_session {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Only a personal administrator may delegate an agent budget".into(),
            ));
        }
    }
    let t = merchant(&h)?;
    permit(&h, "users")?;
    if header(&h, "x-rac-user") == Some("bootstrap") {
        return Err(bad("Personal account required for an integration key"));
    }
    let permissions = validate_permissions(&h, &json!(input.permissions))?;
    if input.agent_budget.is_some() && input.permissions.iter().any(|s| !planning_scope(s)) {
        return Err(Error(StatusCode::FORBIDDEN,"Budgeted planning agents require explicit read scopes; business writes retain personal approval".into()));
    }
    if permissions.as_array().is_none_or(|v| v.is_empty()) {
        return Err(bad("Explicit integration permissions required"));
    }
    let name = Some(input.name.as_str())
        .filter(|s| !s.trim().is_empty() && s.len() <= 100)
        .ok_or(bad("Integration name required"))?;
    if input.non_expiring
        && !verified_kernel::persistent_key_admissible(
            h.principal.personal_session,
            permit(&h, "team.manage").is_ok(),
        )
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "A personal administrator session is required for a non-expiring key".into(),
        ));
    }
    let days = Some(i64::from(input.expires_in_days))
        .filter(|v| (1..=90).contains(v))
        .ok_or(bad("Integration expiry must be 1..90 days"))?;
    let mut tx = a.db.begin().await?;
    veyndo_core::store::tenants::lock(&mut *tx, &t).await?;
    let count = integrations::active_count(&mut tx, &t).await?;
    if count >= 32 {
        return Err(bad("Maximum 32 active integration keys"));
    }
    let id = uid();
    let key = format!("rac_{}{}", uid(), uid());
    integrations::create(
        &mut tx,
        NewKey {
            id: &id,
            tenant: &t,
            user: header(&h, "x-rac-user").unwrap(),
            digest: &hash(&key),
            name,
            permissions: &permissions,
            days: days as i32,
            non_expiring: input.non_expiring,
            agent_budget: input.agent_budget.as_ref().map(|b| json!(b)),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"key":key,"workspace":t})))
}
pub async fn integration_revoke(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "users")?;
    let mut tx = a.db.begin().await?;
    integrations::revoke(&mut tx, &t, &id).await?;
    tx.commit().await?;
    Ok(Json(json!({"revoked":true})))
}
