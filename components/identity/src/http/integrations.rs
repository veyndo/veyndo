//! Explicit-lifetime API/MCP keys are bounded to one workspace and intersect their creator's current membership.
use crate::domain::permissions::planning_scope;
use crate::types::envelopes::Integration;
use crate::{merchant, permit, validate_permissions};
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash, uid};
use veyndo_kernel::verified_kernel;

pub async fn integration_list(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "users")?;
    let rows=sqlx::query("SELECT id,name,user_id,app_id,permissions,agent_budget,agent_tokens_allocated,agent_cost_allocated,created_at::text AS created,CASE WHEN isfinite(expires_at) THEN expires_at::text ELSE NULL END AS expires FROM integration_keys WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100").bind(t).fetch_all(&a.db).await?;
    Ok(Json(
        json!({"elements":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"userId":r.get::<String,_>("user_id"),"appId":r.get::<Option<String>,_>("app_id"),"agentBudget":r.get::<Option<Value>,_>("agent_budget"),"agentUsage":{"tokensAllocated":r.get::<i64,_>("agent_tokens_allocated"),"costMicrosAllocated":r.get::<i64,_>("agent_cost_allocated")},"permissions":r.get::<Value,_>("permissions"),"createdAt":r.get::<String,_>("created"),"expiresAt":r.get::<Option<String>,_>("expires")})).collect::<Vec<_>>()}),
    ))
}
pub async fn integration_create(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Json(input): Json<Integration>,
) -> Result<Json<Value>> {
    let v = json!(input);
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
    let permissions = validate_permissions(&h, &v["permissions"])?;
    if input.agent_budget.is_some() && input.permissions.iter().any(|s| !planning_scope(s)) {
        return Err(Error(StatusCode::FORBIDDEN,"Budgeted planning agents require explicit read scopes; business writes retain personal approval".into()));
    }
    if permissions.as_array().is_none_or(|v| v.is_empty()) {
        return Err(bad("Explicit integration permissions required"));
    }
    let name = v["name"]
        .as_str()
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
    let days = v["expiresInDays"]
        .as_i64()
        .filter(|v| (1..=90).contains(v))
        .ok_or(bad("Integration expiry must be 1..90 days"))?;
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT id FROM tenants WHERE id=$1 FOR UPDATE")
        .bind(&t)
        .fetch_one(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration_keys WHERE tenant=$1 AND expires_at>now()",
    )
    .bind(&t)
    .fetch_one(&mut *tx)
    .await?;
    if count >= 32 {
        return Err(bad("Maximum 32 active integration keys"));
    }
    let id = uid();
    let key = format!("rac_{}{}", uid(), uid());
    sqlx::query("INSERT INTO integration_keys(id,tenant,user_id,digest,name,permissions,expires_at,agent_budget) VALUES($1,$2,$3,$4,$5,$6,CASE WHEN $8 THEN 'infinity'::timestamptz ELSE now()+$7*interval '1 day' END,$9)").bind(&id).bind(&t).bind(header(&h,"x-rac-user").unwrap()).bind(hash(&key)).bind(name).bind(permissions).bind(days as i32).bind(input.non_expiring).bind(input.agent_budget.as_ref().map(|b|json!(b))).execute(&mut *tx).await?;
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
    sqlx::query("UPDATE integration_keys SET expires_at=least(expires_at,now()) WHERE tenant=$1 AND id=$2 AND agent_budget IS NOT NULL").bind(&t).bind(&id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM integration_keys WHERE tenant=$1 AND id=$2 AND agent_budget IS NULL")
        .bind(&t)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"revoked":true})))
}
