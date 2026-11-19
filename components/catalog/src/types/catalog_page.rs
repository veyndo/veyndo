//! The filters and cursor of a storefront catalogue read, and one page of products.
use serde::Deserialize;
use veyndo_core::{Result, bad};
use veyndo_pricing::catalog::Product;

/// What a visitor asks for; the sales channel and the B2B unit are set by the server, never by the request.
#[derive(Default, Deserialize)]
pub struct CatalogCriteria {
    pub limit: Option<u32>,
    pub after: Option<String>,
    pub category: Option<String>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<String>,
    #[serde(alias = "query")]
    pub search: Option<String>,
    #[serde(rename = "ids")]
    pub product_ids: Option<Vec<String>>,
    #[serde(skip)]
    pub channel_id: String,
    #[serde(skip)]
    pub b2b_unit: Option<String>,
}

impl CatalogCriteria {
    pub fn page_size(&self) -> Result<usize> {
        if self.product_ids.as_ref().is_some_and(|ids| {
            ids.len() > 500 || ids.iter().any(|id| id.is_empty() || id.len() > 200)
        }) {
            return Err(bad(
                "Catalog ids must contain at most 500 nonempty product IDs",
            ));
        }
        let limit = self.limit.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(bad("limit must be between 1 and 100"));
        }
        if self.after.as_ref().is_some_and(|s| s.len() > 200)
            || self.category.as_ref().is_some_and(|s| s.len() > 100)
            || self.category_id.as_ref().is_some_and(|s| s.len() > 100)
            || self.search.as_ref().is_some_and(|s| s.len() > 200)
        {
            return Err(bad("Catalog filter is too long"));
        }
        Ok(limit as usize)
    }
}

/// One page in ID order; `next_cursor` is set when more products follow.
pub struct CatalogPage {
    pub products: Vec<Product>,
    pub next_cursor: Option<String>,
    pub limit: usize,
}

#[cfg(test)]
mod tests {
    use super::CatalogCriteria;
    use serde_json::json;
    #[test]
    fn rejects_unbounded_and_invalid_pages() {
        assert_eq!(CatalogCriteria::default().page_size().unwrap(), 50);
        for limit in [0, 101, u32::MAX] {
            assert!(
                CatalogCriteria {
                    limit: Some(limit),
                    ..Default::default()
                }
                .page_size()
                .is_err()
            );
        }
        assert!(
            CatalogCriteria {
                after: Some("x".repeat(201)),
                ..Default::default()
            }
            .page_size()
            .is_err()
        );
        assert!(
            serde_json::from_value::<CatalogCriteria>(json!({"ids":["mug"]}))
                .unwrap()
                .product_ids
                .is_some()
        );
        for ids in [vec![String::new()], vec!["mug".into(); 501]] {
            assert!(
                CatalogCriteria {
                    product_ids: Some(ids),
                    ..Default::default()
                }
                .page_size()
                .is_err()
            );
        }
    }
}
