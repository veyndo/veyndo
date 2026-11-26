//! Reusable input-only selections: their wire form and bounds before current packages resolve them.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_core::{Result, bad};
use veyndo_pricing::cart::Item;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// One saved input-only configuration of a product by an app.
pub struct Selection {
    pub app: String,
    pub product_id: String,
    pub fields: Value,
}
/// Limits saved selections to 100 distinct product and app pairs within 32 KiB.
pub fn selection_shape(items: &[Item], selections: &[Selection]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    if selections.len() > 100
        || json!(selections).to_string().len() > 32768
        || selections.iter().any(|s| {
            !identifier(&s.app)
                || !items.iter().any(|item| item.id == s.product_id)
                || !seen.insert((&s.app, &s.product_id))
                || !s.fields.is_object()
        })
    {
        return Err(bad(
            "Invalid, duplicate or oversized saved configuration selection",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Selection, selection_shape};
    use serde_json::json;
    use veyndo_pricing::cart::Item;

    fn selection(app: &str, product: &str) -> Selection {
        serde_json::from_value(json!({"app":app,"productId":product,"fields":{}})).unwrap()
    }

    #[test]
    fn selections_name_cart_products_once_per_app() {
        let items: Vec<Item> = serde_json::from_value(json!([{"id":"p","quantity":1}])).unwrap();
        assert!(selection_shape(&items, &[selection("a", "p"), selection("b", "p")]).is_ok());
        assert!(selection_shape(&items, &[selection("a", "p"), selection("a", "p")]).is_err());
        assert!(selection_shape(&items, &[selection("a", "other")]).is_err());
        assert!(selection_shape(&items, &[selection("a;drop", "p")]).is_err());
        let mut text = selection("a", "p");
        text.fields = json!("text");
        assert!(selection_shape(&items, &[text]).is_err());
    }
}
