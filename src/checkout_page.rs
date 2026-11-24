//! The existing checkout may be framed only by its registered tenant/channel storefront.
use crate::App;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::Response;
use std::collections::HashMap;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
use veyndo_core::validate_tenant;
pub(crate) async fn page(
    State(a): State<App>,
    h: RequestContext,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response> {
    let mut frames = crate::runtime_config::get().frame_ancestors.clone();
    if q.get("embed").is_some_and(|v| v == "1") {
        let t = q.get("shop").ok_or(bad("Checkout shop required"))?;
        validate_tenant(t)?;
        if tenant(&h)? != *t {
            return Err(bad("Checkout shop scope is inconsistent"));
        }
        let channel = q.get("channel").map(String::as_str).unwrap_or("default");
        let origin = q
            .get("parentOrigin")
            .ok_or(bad("Checkout parent required"))?;
        let mut conn = a.db.begin().await?;
        registered_parent(&mut conn, t, channel, origin).await?;
        frames = origin.clone();
    }
    let html = tokio::fs::read_to_string("frontend/dist/index.html")
        .await
        .map_err(|_| {
            Error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Checkout interface unavailable".into(),
            )
        })?;
    let mut response = Html(html).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    response.headers_mut().insert(
        "content-security-policy",
        crate::security_headers::native_policy(&frames)
            .parse()
            .map_err(|_| bad("Invalid checkout policy"))?,
    );
    Ok(response)
}

pub(crate) use veyndo_checkout::store::hosted_parent::registered_parent;
