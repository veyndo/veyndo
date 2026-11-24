//! Public Ed25519 attestations bind exact native facts and channel context for five minutes; signatures do not guarantee source truth.
use super::evidence;
use crate::marketing;
use crate::{App, commerce};
use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::routing::get;
use serde_json::Value;
use serde_json::json;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
use veyndo_intelligence::service::fact_signing::keys;
pub(crate) use veyndo_intelligence::service::fact_signing::{attest, key_id};
pub(crate) async fn facts(a: &App, h: &RequestContext, id: &str) -> Result<Value> {
    let t = tenant(h)?;
    let detail = commerce::product_detail(
        State(axum::extract::FromRef::from_ref(a)),
        h.clone(),
        Path(id.into()),
        axum::extract::Query(CatalogCriteria::default()),
    )
    .await?
    .0;
    let claims = evidence::claims(a, &t, id, true).await?;
    let now = chrono::Utc::now().timestamp();
    attest(
        json!({"schemaVersion":1,"tenant":t,"channel":marketing::channel_id(h),"issuedAt":now,"expiresAt":now+300,"product":detail["product"],"claims":claims["claims"],"meaning":"Current recorded facts; availability is not a stock reservation and delivery estimates are not guarantees"}),
    )
}
async fn public(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(facts(&a, &h, &id).await?))
}
pub(crate) fn router() -> Router<App> {
    Router::new()
        .route("/.well-known/commerce-facts.json", get(keys))
        .route("/store-api/product/{id}/facts/signed", get(public))
        .route("/ucp/v1/products/{id}/facts", get(public))
}
