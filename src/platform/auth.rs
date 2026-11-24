//! Independent platform authorization: live personal sessions, current grants, no integration/bootstrap escalation.
use crate::auth;
use crate::foundation::App;
use axum::http::StatusCode;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_kernel::verified_kernel;
pub(crate) async fn authenticate(a: &App, h: &RequestContext) -> Result<String> {
    let token = header(h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(Error(
            StatusCode::UNAUTHORIZED,
            "Personal operator login required".into(),
        ))?;
    let (user, _, scopes) = auth::resolve_credential(a, token).await?;
    let active = veyndo_shop::store::operators::grant(a, &user).await?;
    if !verified_kernel::platform_admissible(
        scopes.is_none() && token != *a.token,
        active.is_some(),
        active.unwrap_or(false),
    ) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Platform operator access required".into(),
        ));
    }
    Ok(user)
}
pub(super) use veyndo_shop::http::operator::actor;
