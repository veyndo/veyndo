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
    use super::inherited_extra;
    use serde_json::json;

    #[test]
    fn sku_metadata_inherits_without_overwriting_explicit_values() {
        let parent = json!({"shippingFree":true,"specifications":{"fr":{"Matière":"Grès"}},"crossSelling":["notebook"]});
        assert_eq!(inherited_extra(&parent, &json!({})), parent);
        let child = inherited_extra(&parent, &json!({"shippingFree":false}));
        assert_eq!(child["shippingFree"], false);
        assert_eq!(child["specifications"], parent["specifications"]);
    }
}
