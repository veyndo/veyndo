//! One-use preview handoff becomes a host-only cookie, bound to current session, membership and channel revision.
use crate::service::channel_preview::token;
use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::host_names::links;
use veyndo_core::request_context::HostShop;
use veyndo_core::shop_scope::{channel, header, tenant};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash, uid};
use veyndo_identity::{merchant, permit};
use veyndo_runtime::tenant_scope::{Scope, scoped};

/// The JSON fields `create` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CreateInput {
    #[serde(deserialize_with = "lenient")]
    alias: Option<String>,
}

/// `POST /api/automation/channels/{id}/preview`: a one-use, sixty-second link for the merchant's own browser session.
pub async fn create(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: CreateInput = view(&v);
    permit(&h, "channels.read")?;
    let t = merchant(&h)?;
    let credential = header(&h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(bad("Personal session required"))?;
    let session = hash(credential);
    let exists: bool = scoped(Scope::System,sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_sessions WHERE digest=$1 AND user_id=$2 AND expires_at>now())")
        .bind(&session).bind(header(&h,"x-rac-user")).fetch_one(&a.db)).await?;
    if !exists {
        return Err(bad("Personal session required for a browser preview"));
    }
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM sales_channels WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(&id)
            .fetch_optional(&a.db)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Channel not found".into()))?;
    let alias = input.alias.as_deref();
    let origin = if let Some(alias) = alias {
        let own: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM hosted_frontends WHERE tenant=$1 AND channel=$2 AND alias=$3)")
            .bind(&t).bind(&id).bind(alias).fetch_one(&a.db).await?;
        if !own {
            return Err(bad("Frontend does not belong to this channel"));
        }
        links(alias)["storefrontUrl"].as_str().unwrap().to_owned()
    } else {
        std::env::var("COMMERCE_PUBLIC_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:8787".into())
    };
    let ticket = format!("{}{}", uid(), uid());
    sqlx::query("DELETE FROM channel_previews WHERE expires_at<now() AND tenant=$1")
        .bind(&t)
        .execute(&a.db)
        .await?;
    sqlx::query("INSERT INTO channel_previews(digest,tenant,channel,session_digest,channel_revision,alias,expires_at) VALUES($1,$2,$3,$4,$5,$6,now()+interval '60 seconds')")
        .bind(hash(&ticket)).bind(&t).bind(&id).bind(session).bind(revision).bind(alias).execute(&a.db).await?;
    let origin = reqwest::Url::parse(&origin).map_err(|_| bad("Invalid preview origin"))?;
    Ok(Json(
        json!({"url":format!("{}/channel-preview/{ticket}?shop={}&channel={}",origin.origin().ascii_serialization(),t,id),"expiresIn":60,"previewSeconds":900,"purchases":false}),
    ))
}
/// `GET /channel-preview/{ticket}`: rotates the ticket into a fifteen-minute host-only cookie.
pub async fn redeem(
    State(a): State<veyndo_core::State>,
    Path(ticket): Path<String>,
    request: axum::extract::Request,
) -> Result<Response> {
    let context = RequestContext::from_request(&request);
    let h = &context;
    if !token(&ticket) {
        return Err(bad("Invalid preview link"));
    }
    let t = tenant(h)?;
    let host = request
        .extensions()
        .get::<HostShop>()
        .map(|h| h.alias.as_str());
    let grant = format!("{}{}", uid(), uid());
    // Atomic rotation makes the link one-use; session and revision are rechecked on every subsequent request.
    let row = sqlx::query("UPDATE channel_previews SET digest=$1,redeemed=true,expires_at=now()+interval '15 minutes' WHERE digest=$2 AND tenant=$3 AND channel=$4 AND NOT redeemed AND expires_at>now() AND alias IS NOT DISTINCT FROM $5::text RETURNING alias,channel")
        .bind(hash(&grant)).bind(hash(&ticket)).bind(&t).bind(channel(h)).bind(host).fetch_optional(&a.db).await?
        .ok_or(Error(StatusCode::UNAUTHORIZED,"Preview link expired or already used".into()))?;
    let alias: Option<String> = row.get("alias");
    let target = if alias.is_some() {
        "/?preview=1".to_owned()
    } else {
        format!(
            "/?shop={t}&preview=1&channel={}",
            row.get::<String, _>("channel")
        )
    };
    let mut response = Redirect::to(&target).into_response();
    let secure = if std::env::var("COMMERCE_PUBLIC_ORIGIN")
        .unwrap_or_default()
        .starts_with("https://")
    {
        "; Secure"
    } else {
        ""
    };
    response.headers_mut().insert(
        "set-cookie",
        format!("veyndo_preview={grant}; Path=/; HttpOnly; SameSite=Lax; Max-Age=900{secure}")
            .parse()
            .unwrap(),
    );
    response
        .headers_mut()
        .insert("cache-control", "private, no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    Ok(response)
}

/// `POST /channel-preview/end`: clears the preview cookie.
pub async fn end() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        "set-cookie",
        "veyndo_preview=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            .parse()
            .unwrap(),
    );
    response
        .headers_mut()
        .insert("cache-control", "private, no-store".parse().unwrap());
    response
}
