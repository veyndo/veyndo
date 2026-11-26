//! A variant shows the texts and metadata of its family unless it sets its own.
use serde_json::Value;
use veyndo_pricing::catalog::Product;

/// Absent SKU metadata inherits from the family; explicit values (including false) override.
pub fn inherited_extra(parent: &Value, child: &Value) -> Value {
    let mut merged = parent.as_object().cloned().unwrap_or_default();
    if let Some(fields) = child.as_object() {
        merged.extend(fields.clone());
    }
    Value::Object(merged)
}

/// Gives a product the name, description and inherited metadata of its translated family root.
pub fn inherit(product: &mut Product, root: &Product) {
    product.name = root.name.clone();
    product.description = root.description.clone();
    product.extra = inherited_extra(&root.extra, &product.extra);
}

#[cfg(test)]
mod tests {
    use super::{inherit, inherited_extra};
    use serde_json::{Value, json};
    use veyndo_pricing::catalog::Product;

    fn product(id: &str, name: &str, extra: Value) -> Product {
        serde_json::from_value(json!({
            "extra":extra,"id":id,"name":name,"category":"","description":format!("About {name}"),"price":10.0,
            "tax_rate":19.0,"stock":1,"revision":1,"list_price":null,"regulation_price":null,
            "reference_price":null,"advanced_prices":[],"min_purchase":1,"purchase_steps":1,
            "max_purchase":null,"parent_id":null,"options":{},"media":[],"properties":{},
            "delivery_days":1
        }))
        .unwrap()
    }

    #[test]
    fn a_variant_takes_the_family_texts_and_keeps_its_own_facts() {
        let root = product(
            "family",
            "Mug",
            json!({"shippingFree":true,"seo":{"en":{"title":"Mug"}}}),
        );
        let mut variant = product("family-red", "family-red", json!({"shippingFree":false}));
        variant.price = 12.5;
        inherit(&mut variant, &root);
        assert_eq!(variant.name, "Mug");
        assert_eq!(variant.description, "About Mug");
        assert_eq!(
            variant.extra,
            json!({"shippingFree":false,"seo":{"en":{"title":"Mug"}}})
        );
        assert_eq!((variant.id.as_str(), variant.price), ("family-red", 12.5));
    }

    #[test]
    fn sku_metadata_inherits_without_overwriting_explicit_values() {
        let parent = json!({"shippingFree":true,"specifications":{"fr":{"Matière":"Grès"}},"crossSelling":["notebook"]});
        assert_eq!(inherited_extra(&parent, &json!({})), parent);
        let child = inherited_extra(&parent, &json!({"shippingFree":false}));
        assert_eq!(child["shippingFree"], false);
        assert_eq!(child["specifications"], parent["specifications"]);
    }
}
