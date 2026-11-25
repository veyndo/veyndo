//! Merchant-approved associations are consumed by the public shop without exposing order counts or identities.
use crate::catalog_routes::{CatalogPage, catalog_view};
use crate::foundation::App;
use crate::marketing;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_core::Result;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
pub(crate) use veyndo_intelligence::store::observations::public_pairs;
/// One approved product association, compared exactly as stored.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Pair {
    left: Value,
    right: Value,
}

/// The stock of a catalogue element.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Stocked {
    #[serde(deserialize_with = "lenient")]
    stock: Option<i64>,
}

/// The catalogue page of recommended products with its provenance.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Recommendations {
    #[serde(flatten)]
    page: CatalogPage,
    source: &'static str,
    causal_uplift_proven: bool,
}

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
            let p: Pair = view(p);
            if p.left == id {
                p.right.as_str().map(str::to_string)
            } else if p.right == id {
                p.left.as_str().map(str::to_string)
            } else {
                None
            }
        })
        .take(8)
        .collect::<Vec<_>>();
    let mut page = catalog_view(
        a,
        h,
        CatalogCriteria {
            product_ids: Some(ids),
            limit: Some(8),
            ..Default::default()
        },
    )
    .await?;
    page.elements
        .retain(|p| view::<Stocked>(p).stock.unwrap_or(0) > 0);
    Ok(Json(json!(Recommendations {
        page,
        source: "merchant-approved-order-association",
        causal_uplift_proven: false,
    })))
}

#[cfg(test)]
mod tests {
    use super::{Recommendations, Stocked};
    use crate::catalog_routes::CatalogPage;
    use serde_json::json;
    use veyndo_core::document::view;

    #[test]
    fn recommendation_page_keeps_the_indexed_shape() {
        let page = CatalogPage::sample(vec![
            json!({"id":"a","stock":2}),
            json!({"id":"b","stock":0}),
            json!({"id":"c","stock":1.5}),
        ]);
        let mut old = json!(page);
        if let Some(elements) = old["elements"].as_array_mut() {
            elements.retain(|p| p["stock"].as_i64().unwrap_or(0) > 0);
        }
        old["source"] = json!("merchant-approved-order-association");
        old["causalUpliftProven"] = json!(false);
        let mut page = page;
        page.elements
            .retain(|p| view::<Stocked>(p).stock.unwrap_or(0) > 0);
        let new = json!(Recommendations {
            page,
            source: "merchant-approved-order-association",
            causal_uplift_proven: false,
        });
        assert_eq!(new, old);
    }
}
