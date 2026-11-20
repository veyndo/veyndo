//! App-owned UI surfaces and namespaced HTTP routes reuse the authorized action gateway.
use super::*;
use axum::extract::{OriginalUri, Query};
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/surfaces",
            &[("GET", "catalog.read")],
            get(admin_surfaces),
        )
        .route("/store-api/apps/surfaces", get(public_surfaces))
        .secure_route(
            "/api/apps/{id}/http/{route}",
            &[("GET", "read"), ("POST", "read")],
            get(read_route).post(write_route),
        )
        .route(
            "/store-api/apps/{id}/http/{route}",
            get(read_route).post(write_route),
        )
        .layer(axum::middleware::from_fn(no_store))
}
async fn no_store(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
pub(crate) use veyndo_app_platform::domain::action_policy::readonly;
pub(crate) use veyndo_app_platform::domain::surfaces::validate_contract;
pub(super) fn surface_url(id: &str, path: &str) -> Option<String> {
    veyndo_app_platform::domain::surfaces::surface_url(
        id,
        path,
        &crate::runtime_config::get().services,
    )
}
pub(crate) use super::surface_registry::registry;
async fn admin_surfaces(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(registry(&a, &h, false).await?))
}
async fn public_surfaces(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(registry(&a, &h, true).await?))
}
async fn dispatch(
    a: &App,
    h: &RequestContext,
    id: &str,
    route: &str,
    method: &str,
    public: bool,
    v: &Value,
) -> Result<Value> {
    let m = package(a, &tenant(h)?, id, true).await?;
    let scope = if public { "storefront" } else { "admin" };
    let route = m
        .api_routes
        .iter()
        .find(|r| r.path == route && r.method == method && r.scope == scope)
        .ok_or(Error(
            StatusCode::NOT_FOUND,
            "App route not registered".into(),
        ))?;
    let action = m
        .actions
        .iter()
        .find(|a| a.name == route.action)
        .ok_or(bad("Unknown app action"))?;
    if method == "GET" && !readonly(action) {
        return Err(bad("Read-only app route required"));
    }
    invoke_app(a, h, id, &route.action, v).await
}
async fn read_route(
    State(a): State<App>,
    h: RequestContext,
    OriginalUri(uri): OriginalUri,
    Path((id, route)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>> {
    let m = package(&a, &tenant(&h)?, &id, true).await?;
    let scope = if uri.path().starts_with("/store-api/") {
        "storefront"
    } else {
        "admin"
    };
    let r = m
        .api_routes
        .iter()
        .find(|r| r.path == route && r.method == "GET" && r.scope == scope)
        .ok_or(Error(
            StatusCode::NOT_FOUND,
            "App route not registered".into(),
        ))?;
    let action = m
        .actions
        .iter()
        .find(|a| a.name == r.action)
        .ok_or(bad("Unknown app action"))?;
    let mut input = serde_json::Map::new();
    for (key, value) in query {
        let v = if action.input_schema["properties"][&key]["type"] == "string" {
            json!(value)
        } else {
            serde_json::from_str(&value).map_err(|_| bad("Invalid typed query argument"))?
        };
        input.insert(key, v);
    }
    Ok(Json(
        dispatch(
            &a,
            &h,
            &id,
            &route,
            "GET",
            scope == "storefront",
            &Value::Object(input),
        )
        .await?,
    ))
}
async fn write_route(
    State(a): State<App>,
    h: RequestContext,
    OriginalUri(uri): OriginalUri,
    Path((id, route)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        dispatch(
            &a,
            &h,
            &id,
            &route,
            "POST",
            uri.path().starts_with("/store-api/"),
            &v,
        )
        .await?,
    ))
}
