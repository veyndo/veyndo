//! Operator-pinned, bounded self-contained UI bundles use the same actor/context grant as app actions.
use super::*;
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/surfaces/{surface}/bundle",
            &[("POST", "read")],
            post(load),
        )
        .route("/store-api/apps/{id}/surfaces/{surface}/bundle", post(load))
}
async fn load(
    State(a): State<App>,
    h: RequestContext,
    Path((id, surface)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let (m, s, _, _) = surface_grants::load(&a, &h, &id, &surface, &v).await?;
    let config = &crate::runtime_config::get().services[&id];
    if !approval::approved(&m, config) || native_views::is_native(&m, &s) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "External UI package is not approved".into(),
        ));
    }
    let digest = config["uiDigests"][&s.ui_path]
        .as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "App UI bundle requires an operator-approved SHA-256 digest".into(),
        ))?;
    let url = reqwest::Url::parse(
        &surfaces::surface_url(&id, &s.ui_path).ok_or(bad("App UI URL not configured"))?,
    )
    .map_err(|_| bad("Invalid UI URL"))?;
    let _admission = service_limits::admit(&a, &tenant(&h)?, &id).await?;
    let html = fetch(url, digest).await?;
    Ok(Json(
        json!({"html":html,"digest":digest,"documentUrl":format!("/app-ui/{id}/{digest}"),"sandbox":"opaque-origin; gateway-only actions"}),
    ))
}
pub(super) use veyndo_app_platform::service::ui_bundles::fetch;
