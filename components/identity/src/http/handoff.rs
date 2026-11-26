//! One-time personal-session handoff to the Studio; no passwords or bearer tokens in links.
use crate::permit;
use crate::store::sessions::{SessionProof, issue_verified_tx};
use crate::store::{handoffs, merchant_accounts};
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, hash};

/// `POST /api/auth/handoff`: a one-time link that opens the Studio in the merchant's current session.
pub async fn create(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    permit(&h, "read")?;
    let user = header(&h, "x-rac-user")
        .filter(|u| *u != "bootstrap")
        .ok_or(bad("Personal account required"))?;
    let token = header(&h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(bad("Personal session required"))?;
    Ok(Json(handoffs::create(&a, user, &tenant(&h)?, token).await?))
}
/// The JSON fields `redeem` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RedeemInput {
    #[serde(deserialize_with = "lenient")]
    ticket: Option<String>,
}

/// `POST /api/auth/redeem`: redeems the link once and issues a fresh session for an active member.
pub async fn redeem(Shared(a): Shared<State>, Json(v): Json<Value>) -> Result<Json<Value>> {
    let input: RedeemInput = view(&v);
    let ticket = input
        .ticket
        .as_deref()
        .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or(bad("Invalid login link"))?;
    let mut tx = a.db.begin().await?;
    let user = handoffs::ticket_user(&mut tx, &hash(ticket))
        .await?
        .ok_or(Error(
            StatusCode::UNAUTHORIZED,
            "Login link expired or already used".into(),
        ))?;
    merchant_accounts::lock(&mut tx, &user).await?;
    let row = handoffs::consume(&mut tx, &hash(ticket))
        .await?
        .ok_or(Error(
            StatusCode::UNAUTHORIZED,
            "Login link expired or already used".into(),
        ))?;
    let user = row.user_id;
    let tenant = row.tenant;
    let active = handoffs::member_ready(&mut tx, &user, &tenant).await?;
    if !active {
        return Err(Error(StatusCode::FORBIDDEN, "Shop access revoked".into()));
    }
    let session = issue_verified_tx(&mut tx, &user, &tenant, SessionProof::PersonalHandoff).await?;
    tx.commit().await?;
    Ok(Json(session))
}
