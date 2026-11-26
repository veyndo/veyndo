//! The filters, sort and cursor of the merchant product list, and one listed product.
use serde::Deserialize;
use veyndo_core::{Result, bad, sort_direction};
use veyndo_pricing::catalog::Product;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductCriteria {
    /// Filters contributed by apps; decoded and admitted by the app host.
    #[serde(default)]
    pub app_filter: String,
    pub search: Option<String>,
    pub after: Option<String>,
    pub limit: Option<i64>,
    pub active: Option<bool>,
    pub category_id: Option<String>,
    pub parent_id: Option<String>,
    pub low_stock: Option<bool>,
    #[serde(default)]
    pub preparation: bool,
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub direction: String,
}

impl ProductCriteria {
    /// The page size (default 25, at most 100) after the bounds on all filters were checked.
    pub fn validate(&self) -> Result<i64> {
        let n = self.limit.unwrap_or(25);
        if !(1..=100).contains(&n)
            || self.search.as_ref().is_some_and(|s| s.len() > 200)
            || [&self.after, &self.category_id, &self.parent_id]
                .iter()
                .any(|s| s.as_ref().is_some_and(|v| v.len() > 100))
        {
            return Err(bad("Invalid product filters"));
        }
        self.order()?;
        Ok(n)
    }
    /// The sort expression and direction; only a closed set of columns can be sorted by.
    pub fn order(&self) -> Result<(&'static str, &'static str)> {
        let sort = match self.sort.as_str() {
            "" | "id" => "p.id",
            "name" => "lower(coalesce(grid_tr.name,p.name))",
            "number" => "coalesce(p.product_number,'')",
            "price" => "p.price",
            "stock" => "p.stock",
            _ => return Err(bad("Unsupported product sort")),
        };
        Ok((sort, sort_direction(&self.direction, "ASC")?))
    }
    /// The search text as a `LIKE` pattern with its wildcards escaped.
    pub fn pattern(&self) -> Option<String> {
        self.search.as_ref().map(|s| {
            format!(
                "%{}%",
                s.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        })
    }
}

/// A product of the list with the columns the grid shows beyond the product contract.
pub struct Listed {
    pub product: Product,
    pub active: bool,
    pub product_number: Option<String>,
    pub variant_count: i64,
    pub category_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::ProductCriteria;

    #[test]
    fn the_page_size_and_sort_are_closed() {
        let ok = ProductCriteria::default();
        assert_eq!(ok.validate().unwrap(), 25);
        assert_eq!(ok.order().unwrap(), ("p.id", "ASC"));
        let by_name = ProductCriteria {
            sort: "name".into(),
            direction: "desc".into(),
            ..Default::default()
        };
        assert_eq!(by_name.order().unwrap().1, "DESC");
        for broken in [
            ProductCriteria {
                limit: Some(101),
                ..Default::default()
            },
            ProductCriteria {
                limit: Some(0),
                ..Default::default()
            },
            ProductCriteria {
                sort: "extra; DROP".into(),
                ..Default::default()
            },
            ProductCriteria {
                direction: "sideways".into(),
                ..Default::default()
            },
            ProductCriteria {
                search: Some("x".repeat(201)),
                ..Default::default()
            },
        ] {
            assert!(broken.validate().is_err());
        }
    }

    #[test]
    fn search_wildcards_are_escaped() {
        let c = ProductCriteria {
            search: Some("50%_a\\b".into()),
            ..Default::default()
        };
        assert_eq!(c.pattern().unwrap(), "%50\\%\\_a\\\\b%");
    }
}
