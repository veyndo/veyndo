//! Login, logout and personal workspace discovery.
use crate::domain::credentials::{email, verify_password};
use crate::store::merchant_accounts;
use crate::store::sessions::{self, issue_session, user_data};
use crate::types::envelopes::Login;
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use veyndo_core::document::merge;
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash};

pub async fn user_login(
    State(a): State<veyndo_core::State>,
    Json(input): Json<Login>,
) -> Result<Json<Value>> {
    let v = json!(input);
    let email = email(&v)?;
    let p = input.password.as_str();
    if p.len() > 128 {
        return Err(bad("Invalid credential"));
    }
    let row = merchant_accounts::by_email(&a.db, &email).await?;
    let saved=row.as_ref().map(|r|r.password_hash.clone()).unwrap_or_else(||"$argon2id$v=19$m=19456,t=2,p=1$MTIzNDU2Nzg5MDEyMzQ1Ng$lo5xAWIznNfPKgoIj+puNrTmiUVwvthCpdHQt31aaCk".into());
    if !verify_password(p.into(), saved).await? || row.is_none() {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid credentials".into(),
        ));
    }
    let user = row.unwrap().id;
    let membership = merchant_accounts::first_shop(&a.db, &user)
        .await?
        .ok_or(Error(
            StatusCode::FORBIDDEN,
            "No active workspace membership".into(),
        ))?;
    Ok(Json(issue_session(&a, &user, &membership).await?))
}
pub async fn user_session(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let user = header(&h, "x-rac-user").ok_or(bad("User required"))?;
    if user == "bootstrap" {
        let rows = veyndo_core::store::tenants::all(&a.db).await?;
        return Ok(Json(
            json!({"user":{"id":"bootstrap","email":"","name":"Instance administrator"},"workspaces":rows.iter().map(|(id,name)|json!({"id":id,"name":name,"role":"owner"})).collect::<Vec<_>>(),"workspace":tenant(&h)?}),
        ));
    }
    let mut data = user_data(&a, user).await?;
    merge(&mut data, json!({"workspace":tenant(&h)?}))?;
    Ok(Json(data))
}
pub async fn user_logout(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    if let Some(t) = header(&h, "authorization").and_then(|v| v.strip_prefix("Bearer ")) {
        sessions::end(&a, &hash(t)).await?;
    }
    Ok(Json(json!({"loggedOut":true})))
}
