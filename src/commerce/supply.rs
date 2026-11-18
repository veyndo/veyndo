//! Supplier links for reordering inside the Veyndo network: which product is bought from which shop, when and how much, with a ceiling per order.
use super::*;
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Config {
    /// Ceiling for one reorder in minor units of the pricing currency. Without it nothing can be approved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_order_minor: Option<u64>,
    #[serde(default)]
    pub links: Vec<Link>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Link {
    pub product_id: String,
    pub supplier_shop: String,
    pub supplier_product_id: String,
    pub reorder_point: i32,
    pub reorder_quantity: i32,
}
impl Config {
    pub(crate) fn validate(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        let id = |value: &str| {
            !value.is_empty()
                && value.len() <= 160
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        };
        if self.links.len() > 200
            || self
                .max_order_minor
                .is_some_and(|limit| limit > 1_000_000_000_000)
            || self.links.iter().any(|link| {
                !seen.insert(&link.product_id)
                    || !id(&link.product_id)
                    || !id(&link.supplier_shop)
                    || !id(&link.supplier_product_id)
                    || !(0..=100_000).contains(&link.reorder_point)
                    || !(1..=1000).contains(&link.reorder_quantity)
            })
        {
            return Err(bad(
                "Invalid supplier links: unique products, plain identifiers, reorder point 0–100000 and quantity 1–1000 required",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn link(product: &str, quantity: i32) -> Link {
        Link {
            product_id: product.into(),
            supplier_shop: "supplier".into(),
            supplier_product_id: "lamp".into(),
            reorder_point: 5,
            reorder_quantity: quantity,
        }
    }
    #[test]
    fn links_need_unique_products_plain_ids_and_bounded_quantities() {
        let ok = Config {
            max_order_minor: Some(50_000),
            links: vec![link("a", 10), link("b", 1000)],
        };
        assert!(ok.validate().is_ok());
        assert!(
            Config {
                links: vec![link("a", 10), link("a", 10)],
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                links: vec![link("a", 0)],
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                links: vec![link("a", 1001)],
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                links: vec![link("a/../b", 1)],
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Config {
                max_order_minor: Some(1_000_000_000_001),
                ..ok
            }
            .validate()
            .is_err()
        );
    }
}
