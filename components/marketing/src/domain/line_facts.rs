//! Rule line facts use immutable priced lines plus current tenant product metadata; protected values override metadata.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_pricing::catalog::Product;
/// A priced cart line as the rule facts read it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PricedItem {
    quantity: Value,
    #[serde(deserialize_with = "lenient")]
    price: ItemPrice,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ItemPrice {
    unit_price: Value,
    total_price: Value,
    #[serde(deserialize_with = "lenient")]
    list_price: ListPrice,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListPrice {
    price: Value,
    #[serde(deserialize_with = "lenient")]
    percentage: Option<f64>,
}

/// The product fields that decide a fact the merchant's automation metadata cannot set.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ProductFlags {
    #[serde(deserialize_with = "lenient")]
    shipping_free: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    digital: Option<bool>,
}

/// The facts that come from the product and the priced line; they override any metadata of the same name.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Protected {
    id: String,
    referenced_id: String,
    parent_id: Option<String>,
    quantity: Value,
    r#type: &'static str,
    good: bool,
    stock: i32,
    available_stock: i32,
    unit_price: Value,
    total_price: Value,
    list_price: Value,
    list_price_ratio: f64,
    shipping_free: bool,
    promoted: bool,
    category_ids: Value,
    tax_id: Value,
    option_ids: Value,
    property_ids: Value,
    property_and_option_ids: Vec<Value>,
    volume: f64,
    states: [&'static str; 1],
    product_type: &'static str,
}

/// The values of a stored option or property map, in key order.
fn values(map: &Value) -> Value {
    json!(
        map.as_object()
            .into_iter()
            .flat_map(|o| o.values())
            .cloned()
            .collect::<Vec<_>>()
    )
}

/// One rule fact line: the product's automation metadata with defaults, overridden by the protected product and line values.
pub fn line(p: &Product, item: &Value) -> Value {
    let automation = p.extra.get("automation").and_then(Value::as_object);
    let mut v: Map<String, Value> = automation.cloned().unwrap_or_default();
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
        v.entry(key).or_insert(default);
    }
    let set = |key: &str| automation.and_then(|a| a.get(key)).cloned();
    let item: PricedItem = view(item);
    let flags: ProductFlags = view(&p.extra);
    let option_ids = set("optionIds").unwrap_or_else(|| values(&p.options));
    let property_ids = set("propertyIds").unwrap_or_else(|| values(&p.properties));
    let dimension = |key: &str| v.get(key).and_then(Value::as_f64).unwrap_or(0.);
    let digital = flags.digital == Some(true);
    let protected = Protected {
        id: p.id.clone(),
        referenced_id: p.id.clone(),
        parent_id: p.parent_id.clone(),
        quantity: item.quantity,
        r#type: "product",
        good: true,
        stock: p.stock,
        available_stock: p.stock,
        unit_price: item.price.unit_price,
        total_price: item.price.total_price,
        list_price: item.price.list_price.price,
        list_price_ratio: item
            .price
            .list_price
            .percentage
            .map(|p| (100. - p) / 100.)
            .unwrap_or(0.),
        shipping_free: flags.shipping_free == Some(true),
        promoted: set("markAsTopseller")
            .and_then(|m| m.as_bool())
            .unwrap_or(false),
        category_ids: set("categoryIds").unwrap_or(json!([p.category])),
        tax_id: set("taxId").unwrap_or(json!(p.tax_rate.to_string())),
        property_and_option_ids: property_ids
            .as_array()
            .into_iter()
            .flatten()
            .chain(option_ids.as_array().into_iter().flatten())
            .cloned()
            .collect(),
        option_ids,
        property_ids,
        volume: dimension("width") * dimension("height") * dimension("length") / 1_000_000_000.,
        states: [if digital {
            "is-download"
        } else {
            "is-physical"
        }],
        product_type: if digital { "digital" } else { "physical" },
    };
    if let Ok(Value::Object(fields)) = serde_json::to_value(protected) {
        v.extend(fields);
    }
    Value::Object(v)
}

#[cfg(test)]
mod tests {
    use super::line;
    use serde_json::{Value, json};
    use veyndo_pricing::catalog::Product;

    /// The previous untyped implementation, kept as the oracle of the typed one.
    fn legacy(p: &Product, item: &Value) -> Value {
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

    #[test]
    fn typed_facts_equal_the_untyped_ones() {
        let extras = [
            json!({}),
            json!(null),
            json!({"automation":{"id":"forged","stock":999,"tags":["sale"],"width":100,"height":"2","length":300,"markAsTopseller":"yes","categoryIds":null,"taxId":7,"optionIds":"x","propertyIds":["a"]}}),
            json!({"automation":[1,2],"digital":"true","shippingFree":1}),
            json!({"automation":{"markAsTopseller":true,"volume":5},"digital":true,"shippingFree":true}),
        ];
        let items = [
            json!({}),
            json!(null),
            json!({"quantity":"2","price":{"unitPrice":"x","totalPrice":null,"listPrice":{"price":[1],"percentage":"20"}}}),
            json!({"quantity":2,"price":{"unitPrice":10.0,"totalPrice":20.0,"listPrice":{"price":12.5,"percentage":20}}}),
            json!({"price":[1,2,3]}),
            json!({"price":{"listPrice":[12.5,20.0]}}),
        ];
        for extra in &extras {
            for item in &items {
                let p = product(extra.clone());
                assert_eq!(line(&p, item), legacy(&p, item), "{extra} {item}");
            }
        }
    }

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
