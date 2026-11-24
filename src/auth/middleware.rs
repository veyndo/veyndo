//! Resolve sessions from PostgreSQL on every request: role changes/revocation work across replicas.
//! Remove all client-supplied principal headers before adding server-derived identities.
use crate::{App, apps};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::{extract::Request, middleware::Next};
use veyndo_core::Error;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_core::shop_scope::tenant;
fn forbidden(s: &str) -> Error {
    Error(StatusCode::FORBIDDEN, s.into())
}
/// The Checkout page names its shop in the query; it may not contradict a selected shop.
fn checkout_shop(request: &Request, h: &mut RequestContext, path: &str) -> Result<(), Error> {
    if path == "/checkout"
        && let Ok(url) = reqwest::Url::parse(&format!("http://local{}", request.uri()))
        && let Some((_, shop)) = url.query_pairs().find(|(k, _)| k == "shop")
    {
        if veyndo_core::validate_tenant(&shop).is_err()
            || header(h, "x-tenant").is_some_and(|selected| selected != shop)
        {
            return Err(bad("Checkout shop scope is inconsistent"));
        }
        match shop.parse() {
            Ok(value) => {
                h.insert("x-tenant", value);
            }
            Err(_) => return Err(bad("Invalid checkout shop scope")),
        }
    }
    Ok(())
}
/// Image and preview subrequests carry their shop and channel in the query instead of headers.
fn asset_scope(request: &Request, h: &mut RequestContext, path: &str) -> Result<(), Error> {
    // Public image subrequests cannot carry frontend custom headers. Resolve only the scoped asset URL
    // before sandbox/session admission; private shops still require the same merchant credential.
    if (path.starts_with("/store-api/assets/")
        || path.starts_with("/api/merchant/assets/")
        || path.starts_with("/store-api/company-logo/")
        || path.starts_with("/products/")
        || path.starts_with("/channel-preview/"))
        && header(h, "x-tenant").is_none()
        && let Ok(url) = reqwest::Url::parse(&format!("http://local{}", request.uri()))
        && let Some((_, shop)) = url.query_pairs().find(|(k, _)| k == "shop")
    {
        match shop.parse() {
            Ok(value) => {
                h.insert("x-tenant", value);
            }
            Err(_) => return Err(bad("Invalid asset shop scope")),
        }
    }
    if (path.starts_with("/channel-preview/") || path.starts_with("/store-api/assets/"))
        && request
            .extensions()
            .get::<crate::shop_domains::HostShop>()
            .is_none()
        && let Ok(url) = reqwest::Url::parse(&format!("http://local{}", request.uri()))
        && let Some((_, channel)) = url.query_pairs().find(|(k, _)| k == "channel")
    {
        if !apps::identifier(&channel) {
            return Err(bad("Invalid preview channel"));
        }
        h.insert("sw-sales-channel-id", channel.parse().unwrap());
    }
    Ok(())
}
/// The database scope of the admitted request: system for identity and provisioning, otherwise the shop and its actor.
fn request_scope(
    h: &RequestContext,
    path: &str,
    public: bool,
) -> Result<veyndo::tenant_scope::Scope, Error> {
    Ok(
        if path.starts_with("/api/auth/")
            || path.starts_with("/api/identity/")
            || path == "/api/workspaces"
            || path.starts_with("/api/developer")
            || path.starts_with("/api/environments")
            || path.starts_with("/api/workspace/")
        {
            // Provisioning, identity and staging require cross-workspace reads, after their existing grants.
            veyndo::tenant_scope::Scope::System
        } else {
            match tenant(h) {
                Ok(t) => match h.principal.user.as_ref() {
                    Some(actor) if h.principal.personal_session => {
                        veyndo::tenant_scope::Scope::Personal {
                            tenant: t,
                            actor: actor.clone(),
                        }
                    }
                    Some(actor) => veyndo::tenant_scope::Scope::Actor {
                        tenant: t,
                        actor: actor.clone(),
                    },
                    None => veyndo::tenant_scope::Scope::Tenant(t),
                },
                Err(_) if public => veyndo::tenant_scope::Scope::Denied,
                Err(e) => return Err(e),
            }
        },
    )
}
pub(crate) use veyndo_identity::permit;
pub(crate) async fn authenticate(state: State<App>, request: Request, next: Next) -> Response {
    // Identity lookup is trusted; the admitted business request gets a narrower context below.
    veyndo::tenant_scope::scoped(
        veyndo::tenant_scope::Scope::System,
        authenticate_scoped(state, request, next),
    )
    .await
}
async fn authenticate_scoped(State(a): State<App>, mut request: Request, next: Next) -> Response {
    for key in [
        "x-rac-user",
        "x-rac-role",
        "x-rac-tenant",
        "x-rac-permissions",
        "x-rac-platform-user",
        "x-rac-history-reason",
        "x-rac-channel-preview",
    ] {
        request.headers_mut().remove(key);
    }
    let mut h = RequestContext::from_request(&request);
    let path = request.uri().path().to_string();
    let method = request.method().to_string();
    if let Err(e) = checkout_shop(&request, &mut h, &path) {
        return e.into_response();
    }
    if let Err(e) = apps::bind_webhook_scope(&path, &mut h) {
        return e.into_response();
    }
    // Only the public native shell/assets bypass identity. Hosted shop assets still
    // require current tenant/channel admission and authorized private previews.
    if veyndo_kernel::verified_kernel::native_asset_bypass(
        crate::performance::delivery::native_static(&path),
        request
            .extensions()
            .get::<crate::shop_domains::HostShop>()
            .is_some(),
    ) {
        request.extensions_mut().insert(h);
        return next.run(request).await;
    }
    if let Err(e) = asset_scope(&request, &mut h, &path) {
        return e.into_response();
    }
    if path.starts_with("/api/platform/")
        && request
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .and_then(|p| super::route_policy::lookup(p.as_str(), &method))
            != Some("platform")
    {
        return forbidden("Platform route has no registered permission").into_response();
    }
    // Global administration has its own personal-session grant; tenant/admin/bootstrap roles cannot inherit it.
    if path.starts_with("/api/platform/") {
        match crate::platform::authenticate(&a, &h).await {
            Ok(user) => {
                h.insert("x-rac-platform-user", user.parse().unwrap());
            }
            Err(e) => return e.into_response(),
        }
        request.extensions_mut().insert(h);
        return next.run(request).await;
    }

    let policy = if crate::shop_domains::frontends::delegated_api(&request) {
        // Public frontend endpoints still pass identity, tenant/channel admission
        // and resource limits below; no merchant credential reaches the proxy.
        "public"
    } else if path.starts_with("/api/") {
        let matched = request
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .map(|p| p.as_str());
        match matched.and_then(|p| super::route_policy::lookup(p, &method)) {
            Some(policy) => policy,
            None => return forbidden("API route has no registered permission").into_response(),
        }
    } else {
        "public"
    };
    let public = policy == "public";
    let protected = path.starts_with("/api/") && !public;
    let auth_result = async {
        // Introspection validates its own service credential before resolving the grant's tenant.
        if method != "POST"
            || !matches!(
                path.as_str(),
                crate::checkout::buyer_identity::INTROSPECTION
                    | super::knowledge_identity::INTROSPECTION
            )
        {
            super::identity::resolve(&a, &mut h, &path, protected).await?;
        }
        if protected {
            permit(&h, policy)?;
        }
        Ok::<(), Error>(())
    }
    .await;
    if let Err(e) = auth_result {
        return e.into_response();
    }
    if let Err(e) = crate::agent_runs::controls::admit_key(&a, &h, &path, &method).await {
        return e.into_response();
    }
    h.validated_tenant = match crate::platform::admit(&a, &h, &path, &method).await {
        Ok(validated) => validated || h.principal.tenant.is_some(),
        Err(e) => return e.into_response(),
    };
    let preview = match crate::marketing::channel_access::admit(
        &a,
        &h,
        &path,
        &method,
        request
            .extensions()
            .get::<crate::shop_domains::HostShop>()
            .map(|h| h.alias.as_str()),
    )
    .await
    {
        Ok(preview) => preview,
        Err(e) => return e.into_response(),
    };
    if preview {
        let channel = crate::marketing::channel_id(&h).to_owned();
        h.insert("x-rac-channel-preview", channel.parse().unwrap());
    }
    let private_response = path.starts_with("/api/auth/")
        || path.starts_with("/api/identity/")
        || preview
        || header(&h, "x-rac-role").is_some()
        || request
            .extensions()
            .get::<crate::shop_domains::HostShop>()
            .is_some();
    let scope = match request_scope(&h, &path, public) {
        Ok(scope) => scope,
        Err(e) => return e.into_response(),
    };
    *request.headers_mut() = h.transport().clone();
    let agent_authority = crate::agent_runs::controls::model_authority(&h);
    request.extensions_mut().insert(h);
    let mut response = veyndo::inference::budget::scoped(
        agent_authority,
        veyndo::tenant_scope::scoped(scope, veyndo_identity::http::abuse::run(&a, request, next)),
    )
    .await;
    if private_response {
        response
            .headers_mut()
            .insert("cache-control", "private, no-store".parse().unwrap());
    }
    response
}
