//! Package transport: capability discovery for agents, switching an installed package on or off, and the public consent adapter.
use crate::ports::Platform;
use crate::service::discovery;
use crate::store::activation;
use axum::Json;
use axum::extract::{Path, State as Shared};
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::Value;
use veyndo_app_storage::Scope;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, bad};
use veyndo_identity::{merchant, permit};

/// Searches the app capabilities the caller may use.
pub async fn search(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        discovery::invoke(&a, &h, "merchant.apps.search", &v).await?,
    ))
}

/// Reads through one admitted managed action of an installed app.
pub async fn read(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        discovery::invoke(&a, &h, "merchant.apps.read", &v).await?,
    ))
}

/// The JSON fields `state` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StateInput {
    #[serde(deserialize_with = "lenient")]
    active: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Activates or deactivates an installed package under optimistic revision control.
pub async fn state(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: StateInput = view(&v);
    permit(&h, "users")?;
    let t = merchant(&h)?;
    let active = input.active.ok_or(bad("active required"))?;
    let mut tx = a.db.begin().await?;
    let result = veyndo_app_storage::activation::set(
        &mut tx,
        &veyndo_app_storage::activation::Change {
            scope: Scope {
                tenant: &t,
                app: &id,
            },
            active,
            revision: input.revision,
        },
        &activation::Host {
            tenant: &t,
            app: &id,
            actor: &h,
            commerce: a.commerce,
        },
    )
    .await
    .map_err(activation::error)?;
    tx.commit().await?;
    Ok(Json(result))
}

/// The shared public consent adapter; it contains no shop configuration or provider credentials.
pub async fn analytics_sdk() -> impl IntoResponse {
    (
        [
            ("content-type", "text/javascript; charset=utf-8"),
            ("cache-control", "public, max-age=3600"),
            ("x-content-type-options", "nosniff"),
        ],
        include_str!("../../../../extensions/sdk/analytics.js"),
    )
}

#[cfg(test)]
mod tests {
    use super::analytics_sdk;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn the_consent_adapter_is_served_as_cacheable_script() {
        let response = analytics_sdk().await.into_response();
        let headers = response.headers();
        assert_eq!(headers["content-type"], "text/javascript; charset=utf-8");
        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert_eq!(headers["cache-control"], "public, max-age=3600");
    }
}
