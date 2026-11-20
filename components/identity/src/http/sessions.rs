//! Login, logout and personal workspace discovery.
use crate::domain::credentials::{email, verify_password};
use crate::store::sessions::{issue_session, user_data};
use crate::types::envelopes::Login;
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash};

pub async fn user_login(
    State(a): State<veyndo_core::State>,
    Json(input): Json<Login>,
) -> Result<Json<Value>> {
    let v = json!(input);
    let email = email(&v)?;
    let p = v["password"].as_str().unwrap_or("");
    if p.len() > 128 {
        return Err(bad("Invalid credential"));
    }
    let row = sqlx::query("SELECT id,password_hash FROM merchant_users WHERE email=$1")
        .bind(email)
        .fetch_optional(&a.db)
        .await?;
    let saved=row.as_ref().map(|r|r.get::<String,_>("password_hash")).unwrap_or_else(||"$argon2id$v=19$m=19456,t=2,p=1$MTIzNDU2Nzg5MDEyMzQ1Ng$lo5xAWIznNfPKgoIj+puNrTmiUVwvthCpdHQt31aaCk".into());
    if !verify_password(p.into(), saved).await? || row.is_none() {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid credentials".into(),
        ));
    }
    let user = row.unwrap().get::<String, _>("id");
    let membership = sqlx::query(
        "SELECT tenant FROM membership_grants WHERE user_id=$1 AND active ORDER BY tenant LIMIT 1",
    )
    .bind(&user)
    .fetch_optional(&a.db)
    .await?
    .ok_or(Error(
        StatusCode::FORBIDDEN,
        "No active workspace membership".into(),
    ))?;
    Ok(Json(
        issue_session(&a, &user, &membership.get::<String, _>("tenant")).await?,
    ))
}
pub async fn user_session(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let user = header(&h, "x-rac-user").ok_or(bad("User required"))?;
    if user == "bootstrap" {
        let rows = sqlx::query("SELECT id,name FROM tenants ORDER BY name")
            .fetch_all(&a.db)
            .await?;
        return Ok(Json(
            json!({"user":{"id":"bootstrap","email":"","name":"Instance administrator"},"workspaces":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"role":"owner"})).collect::<Vec<_>>(),"workspace":tenant(&h)?}),
        ));
    }
    let mut data = user_data(&a, user).await?;
    data["workspace"] = json!(tenant(&h)?);
    Ok(Json(data))
}
pub async fn user_logout(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    if let Some(t) = header(&h, "authorization").and_then(|v| v.strip_prefix("Bearer ")) {
        sqlx::query("DELETE FROM user_sessions WHERE digest=$1")
            .bind(hash(t))
            .execute(&a.db)
            .await?;
    }
    Ok(Json(json!({"loggedOut":true})))
}
