//! Customer-bound, short-lived provider UI sessions; iframe messages are never ledger receipts.
use crate::domain::receipt_ref::identity;
use crate::domain::service_config::approval_url;
use crate::ports::{Gateway, Host};
use crate::service::remote::execute;
use crate::store::attempts;
use crate::store::commands::customer;
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use veyndo_core::document::view;
use veyndo_core::host_names::{configured_alias, same_origin_family};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, uid};
use veyndo_shop::store::hosted_bindings::bound as hosted_alias;

/// The provider's embedded checkout session.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SessionAnswer {
    ui_url: Value,
    session_token: Value,
    expires_in: Value,
    nonce: Value,
    ready_signal: Value,
}

pub async fn session(
    host: &dyn Host,
    gateway: &dyn Gateway,
    a: &State,
    h: &RequestContext,
    id: &str,
    parent_origin: Option<&str>,
) -> Result<Value> {
    customer(a, h, id).await?;
    let t = tenant(h)?;
    let p = attempts::get(&a.db, &t, id).await?;
    let ctx = p.ctx();
    if p.provider == "paypal"
        || ctx.checkout != "embedded"
        || !["ready", "approved"].contains(&p.state.as_str())
    {
        return Err(conflict("Embedded payment is not available in this state"));
    }
    let parent = origin(
        a,
        &t,
        ctx.channel.as_str().unwrap_or("default"),
        parent_origin,
    )
    .await?;
    let nonce = uid();
    let v = execute(
        gateway,
        a,
        &p,
        "checkout_session",
        &nonce,
        &json!({"nonce":nonce,"parentOrigin":parent}),
    )
    .await?;
    identity(&p, &v)?;
    let answer: SessionAnswer = view(&v);
    let url = approval_url(host, &p, &answer.ui_url)?.ok_or(bad("Checkout UI URL required"))?;
    if reqwest::Url::parse(&url)
        .map_err(|_| bad("Invalid checkout origin"))?
        .origin()
        == reqwest::Url::parse(&parent)
            .map_err(|_| bad("Invalid commerce origin"))?
            .origin()
    {
        return Err(bad("Provider UI must use a separate origin"));
    }
    let token = answer
        .session_token
        .as_str()
        .filter(|s| s.len() >= 24 && s.len() <= 2048)
        .ok_or(bad("Bounded checkout session token required"))?;
    let ttl = answer
        .expires_in
        .as_u64()
        .filter(|n| *n > 0 && *n <= 600)
        .ok_or(bad("Short-lived checkout session required"))?;
    if answer.nonce != nonce {
        return Err(bad("Checkout session nonce mismatch"));
    }
    Ok(
        json!({"uiUrl":url,"sessionToken":token,"nonce":nonce,"expiresIn":ttl,"readySignal":answer.ready_signal.as_bool().unwrap_or(false)}),
    )
}

/// Browser origins are accepted only from operator configuration or the tenant's owned channel address.
pub async fn origin(a: &State, t: &str, channel: &str, requested: Option<&str>) -> Result<String> {
    let mut tx = a.db.begin().await?;
    origin_conn(&mut tx, t, channel, requested).await
}
pub async fn origin_conn(
    conn: &mut sqlx::PgConnection,
    t: &str,
    channel: &str,
    requested: Option<&str>,
) -> Result<String> {
    let core = env::var("COMMERCE_PUBLIC_ORIGIN")
        .or_else(|_| env::var("PUBLIC_BASE_URL"))
        .unwrap_or("http://127.0.0.1:8787".into());
    let supplied = requested.unwrap_or(&core);
    let u = reqwest::Url::parse(supplied).map_err(|_| bad("Invalid checkout parent origin"))?;
    let normalized = u.origin().ascii_serialization();
    if !["http", "https"].contains(&u.scheme())
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || u.path() != "/"
    {
        return Err(bad("Checkout parent must be an origin"));
    }
    let suffix = env::var("SHOP_DOMAIN_SUFFIX").unwrap_or_default();
    let legacy = env::var("LEGACY_DOMAINS").unwrap_or_default();
    if reqwest::Url::parse(&core).is_ok_and(|c| same_origin_family(&u, &c, &suffix, &legacy)) {
        return Ok(normalized);
    }
    if u.scheme() == "https"
        && u.port().is_none()
        && let Ok(suffix) = env::var("SHOP_DOMAIN_SUFFIX")
    {
        let host = u.host_str().unwrap_or("");
        if let Some(alias) = configured_alias(host, &suffix, &legacy) {
            if alias == t {
                return Ok(normalized);
            }
            if hosted_alias(conn, t, channel, alias).await? {
                return Ok(normalized);
            }
        }
    }
    Err(Error(
        StatusCode::FORBIDDEN,
        "Checkout origin does not belong to this shop/channel".into(),
    ))
}
