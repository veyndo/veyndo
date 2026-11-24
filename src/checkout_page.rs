//! The existing checkout may be framed only by its registered tenant/channel storefront.
use crate::{
    App, Error, HashMap, IntoResponse, RequestContext, Response, Result, State, StatusCode, bad,
    tenant, validate_tenant,
};
use axum::extract::Query;
use axum::response::Html;
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
