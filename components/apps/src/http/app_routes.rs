//! Namespaced HTTP routes an app declares reuse the authorized action gateway.
use crate::domain::action_policy::readonly;
use crate::ports::Platform;
use crate::service::gateway;
use crate::store::packages::package;
use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State as Shared};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};

async fn dispatch(
    a: &Platform,
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
    gateway::invoke(a, h, id, &route.action, v, None).await
}
/// Serves a declared GET route of an app through a read-only action.
pub async fn read_route(
    Shared(a): Shared<Platform>,
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
/// Serves a declared POST route of an app.
pub async fn write_route(
    Shared(a): Shared<Platform>,
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
