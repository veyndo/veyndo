//! Durable long actions piggyback on the existing outbox. Apps claim a fenced lease; unknown side effects are never retried automatically.
use super::*;
pub(super) async fn enqueue(
    a: &App,
    h: &RequestContext,
    m: &Manifest,
    action: &Action,
    v: &Value,
) -> Result<Value> {
    veyndo_app_platform::store::jobs::enqueue(a, h, m, action, v).await
}
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/jobs",
            &[("GET", "apps.manage")],
            get(veyndo_app_platform::http::jobs::list),
        )
        .secure_route(
            "/api/apps/{id}/jobs/{job}",
            &[("GET", "read"), ("POST", "apps.manage")],
            get(veyndo_app_platform::http::jobs::detail)
                .post(veyndo_app_platform::http::jobs::control),
        )
}
