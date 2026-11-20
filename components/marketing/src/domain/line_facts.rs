//! Rule line facts use immutable priced lines plus current tenant product metadata; protected values override metadata.
use serde_json::{Value, json};
use veyndo_pricing::catalog::Product;
pub fn line(p: &Product, item: &Value) -> Value {
    let a = &p.extra["automation"];
    let mut v = if a.is_object() { a.clone() } else { json!({}) };
    for (key, default) in [
        ("height", json!(0)),
        ("width", json!(0)),
        ("length", json!(0)),
        ("weight", json!(0)),
        ("volume", json!(0)),
        ("closeout", json!(false)),
        ("isNew", json!(false)),
        ("tags", json!([])),
        ("streamIds", json!([])),
        ("manufacturerId", Value::Null),
        ("customFields", json!({})),
        ("purchasePrices", Value::Null),
        ("createdAt", Value::Null),
        ("releaseDate", Value::Null),
    ] {
        if v.get(key).is_none() {
            v[key] = default
        }
    }
    v["id"] = json!(p.id);
    v["referencedId"] = json!(p.id);
    v["parentId"] = json!(p.parent_id);
    v["quantity"] = item["quantity"].clone();
    v["type"] = json!("product");
    v["good"] = json!(true);
    v["stock"] = json!(p.stock);
    v["availableStock"] = json!(p.stock);
    v["unitPrice"] = item["price"]["unitPrice"].clone();
    v["totalPrice"] = item["price"]["totalPrice"].clone();
    v["listPrice"] = item["price"]["listPrice"]["price"].clone();
    v["listPriceRatio"] = json!(
        item["price"]["listPrice"]["percentage"]
            .as_f64()
            .map(|p| (100. - p) / 100.)
            .unwrap_or(0.)
    );
    v["shippingFree"] = json!(p.extra["shippingFree"] == true);
    v["promoted"] = json!(a["markAsTopseller"].as_bool().unwrap_or(false));
    v["categoryIds"] = a.get("categoryIds").cloned().unwrap_or(json!([p.category]));
    v["taxId"] = a
        .get("taxId")
        .cloned()
        .unwrap_or(json!(p.tax_rate.to_string()));
    v["optionIds"] = a.get("optionIds").cloned().unwrap_or(json!(
        p.options
            .as_object()
            .into_iter()
            .flat_map(|o| o.values())
            .cloned()
            .collect::<Vec<_>>()
    ));
    v["propertyIds"] = a.get("propertyIds").cloned().unwrap_or(json!(
        p.properties
            .as_object()
            .into_iter()
            .flat_map(|o| o.values())
            .cloned()
            .collect::<Vec<_>>()
    ));
    v["propertyAndOptionIds"] = json!(
        v["propertyIds"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(v["optionIds"].as_array().into_iter().flatten())
            .cloned()
            .collect::<Vec<_>>()
    );
    v["volume"] = json!(
        v["width"].as_f64().unwrap_or(0.)
            * v["height"].as_f64().unwrap_or(0.)
            * v["length"].as_f64().unwrap_or(0.)
            / 1_000_000_000.
    );
    v["states"] = json!([if p.extra["digital"] == true {
        "is-download"
    } else {
        "is-physical"
    }]);
    v["productType"] = json!(if p.extra["digital"] == true {
        "digital"
    } else {
        "physical"
    });
    v
}

#[cfg(test)]
mod tests {
    use super::line;
    use serde_json::{Value, json};
    use veyndo_pricing::catalog::Product;

    fn product(extra: Value) -> Product {
        serde_json::from_value(json!({
            "extra":extra,"id":"p1","name":"Lamp","category":"lighting","description":"","price":10.0,
            "tax_rate":19.0,"stock":4,"revision":1,"list_price":null,"regulation_price":null,
            "reference_price":null,"advanced_prices":[],"min_purchase":1,"purchase_steps":1,
            "max_purchase":null,"parent_id":"parent-1","options":{"color":"red"},"media":[],
            "properties":{"material":"steel"},"delivery_days":1
        }))
        .unwrap()
    }

    #[test]
    fn protected_values_come_from_the_product_and_the_priced_line() {
        let item = json!({"quantity":2,"price":{"unitPrice":10.0,"totalPrice":20.0,"listPrice":{"price":12.5,"percentage":20.0}}});
        let v = line(
            &product(
                json!({"automation":{"id":"forged","stock":999,"tags":["sale"],"width":100,"height":200,"length":300}}),
            ),
            &item,
        );
        assert_eq!(v["id"], "p1");
        assert_eq!(v["referencedId"], "p1");
        assert_eq!(v["parentId"], "parent-1");
        assert_eq!(v["stock"], 4);
        assert_eq!(v["availableStock"], 4);
        assert_eq!(v["quantity"], 2);
        assert_eq!(v["unitPrice"], 10.0);
        assert_eq!(v["totalPrice"], 20.0);
        assert_eq!(v["listPrice"], 12.5);
        assert_eq!(v["listPriceRatio"], 0.8);
        assert_eq!(v["tags"], json!(["sale"]));
        assert_eq!(v["volume"], 0.006);
        assert_eq!(v["type"], "product");
    }

    #[test]
    fn missing_metadata_gets_defaults_derived_from_the_product() {
        let v = line(&product(json!({})), &json!({"quantity":1}));
        assert_eq!(v["categoryIds"], json!(["lighting"]));
        assert_eq!(v["taxId"], "19");
        assert_eq!(v["optionIds"], json!(["red"]));
        assert_eq!(v["propertyIds"], json!(["steel"]));
        assert_eq!(v["propertyAndOptionIds"], json!(["steel", "red"]));
        assert_eq!(v["listPriceRatio"], 0.0);
        assert_eq!(v["closeout"], false);
        assert_eq!(v["customFields"], json!({}));
        assert_eq!(v["shippingFree"], false);
        assert_eq!(v["promoted"], false);
        assert_eq!(v["states"], json!(["is-physical"]));
        assert_eq!(v["productType"], "physical");
        let digital = line(
            &product(
                json!({"digital":true,"shippingFree":true,"automation":{"markAsTopseller":true,"categoryIds":["c9"]}}),
            ),
            &json!({}),
        );
        assert_eq!(digital["states"], json!(["is-download"]));
        assert_eq!(digital["productType"], "digital");
        assert_eq!(digital["shippingFree"], true);
        assert_eq!(digital["promoted"], true);
        assert_eq!(digital["categoryIds"], json!(["c9"]));
    }
}
