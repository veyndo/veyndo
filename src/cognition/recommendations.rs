//! Merchant-approved associations are consumed by the public shop without exposing order counts or identities.
use crate::{App, catalog_page, marketing};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
pub(crate) use veyndo_intelligence::store::observations::public_pairs;
pub(crate) async fn recommendations(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = tenant(&h)?;
    marketing::admit_product(&a, &h, &id).await?;
    let pairs = public_pairs(&a, &t).await?;
    let ids = pairs
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| {
            if p["left"] == id {
                p["right"].as_str().map(str::to_string)
            } else if p["right"] == id {
                p["left"].as_str().map(str::to_string)
            } else {
                None
            }
        })
        .take(8)
        .collect::<Vec<_>>();
    let Json(mut result) = catalog_page(
        State(a),
        h,
        CatalogCriteria {
            product_ids: Some(ids),
            limit: Some(8),
            ..Default::default()
        },
    )
    .await?;
    if let Some(elements) = result["elements"].as_array_mut() {
        elements.retain(|p| p["stock"].as_i64().unwrap_or(0) > 0);
    }
    result["source"] = json!("merchant-approved-order-association");
    result["causalUpliftProven"] = json!(false);
    Ok(Json(result))
}
