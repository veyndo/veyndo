//! Catalogue intake routes and the server side of the intake port; the intelligence component owns drafts, extraction and the approved sandbox save.
use crate::{App, SecureRoutes, apps, commerce};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
mod schema;
pub(crate) use schema::{CAPABILITIES, schema};
use veyndo_core::PortFuture;
use veyndo_intelligence::ports::PublicEgress;
pub(crate) use veyndo_intelligence::service::catalog_intake::visible;
use veyndo_intelligence::service::models::Models;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/merchant/catalog/intake",
            &[("GET", "catalog.read"), ("POST", "catalog.write")],
            get(context).post(prepare_route),
        )
        .secure_route(
            "/api/merchant/catalog/intake/save",
            &[("POST", "catalog.write")],
            post(save_route),
        )
        .layer(axum::extract::DefaultBodyLimit::max(300_000))
}
async fn context(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "merchant.catalog.context", &json!({})).await?,
    ))
}
async fn prepare_route(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "merchant.catalog.prepare", &v).await?))
}
async fn save_route(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "merchant.catalog.save", &v).await?))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::catalog_intake::invoke(&Intake(a), h, name, v).await
}

/// Models, egress, provider admission, readiness and the native product editor of this process.
struct Intake<'s>(&'s App);
impl veyndo_intelligence::ports::IntakeHost for Intake<'_> {
    fn models(&self) -> Models<'_> {
        self.0.models()
    }
    fn egress(&self) -> &dyn PublicEgress {
        &crate::assets::Egress
    }
    fn admit<'a>(
        &'a self,
        tenant: &'a str,
        provider: &'a str,
    ) -> PortFuture<'a, Box<dyn Send + Sync>> {
        Box::pin(async move {
            let guard = apps::service_limits::admit(self.0, tenant, provider).await?;
            Ok(Box::new(guard) as Box<dyn Send + Sync>)
        })
    }
    fn readiness<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, Value> {
        Box::pin(crate::ucp::readiness(self.0, h))
    }
    fn create<'a>(&'a self, h: &'a RequestContext, product: Value) -> PortFuture<'a, Value> {
        Box::pin(async move {
            Ok(
                commerce::create_product(State(self.0.core.clone()), h.clone(), Json(product))
                    .await?
                    .0,
            )
        })
    }
}
