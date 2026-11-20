//! Explicit catalogue-only preview: whitelist current mapped/native fields, bound text/media, reuse native money formatting.
use serde_json::{Value, json};
use veyndo_core::{Result, bad};
pub fn projection(value: &Value, scale: u8) -> Result<Value> {
    let code = value["extra"]["priceCurrency"].as_str().unwrap_or("EUR");
    let currency = veyndo_pricing::money::Currency::new(code, scale)
        .map_err(|_| bad("Invalid product currency"))?;
    let amount =
        veyndo_pricing::money::Money::rounded(&value["commerce"]["price"].to_string(), currency)
            .map_err(|_| bad("Invalid native product price"))?
            .decimal();
    let mut translations = serde_json::Map::new();
    for (locale, fields) in value["translations"]
        .as_object()
        .into_iter()
        .flatten()
        .take(16)
    {
        translations.insert(
            locale.clone(),
            json!({"name":text(&fields["name"]),"description":text(&fields["description"])}),
        );
    }
    let media: Vec<Value> = value["commerce"]["media"]
        .as_array()
        .into_iter()
        .flatten()
        .take(8)
        .map(|item| json!({"url":text(&item["url"])}))
        .collect();
    Ok(
        json!({"translations":translations,"sku":text(&value["catalog"]["productNumber"]),"active":value["catalog"]["active"],"parentId":value["catalog"]["parentId"],"options":value["catalog"]["options"],"price":{"amount":amount,"currency":code},"taxRate":value["commerce"]["taxRate"],"media":media}),
    )
}
fn text(v: &Value) -> String {
    v.as_str().unwrap_or("").chars().take(4000).collect()
}

#[cfg(test)]
mod tests {
    use super::projection;
    use serde_json::json;

    #[test]
    fn the_preview_keeps_whitelisted_fields_and_formats_the_price_in_its_currency() {
        let value = json!({"extra":{"priceCurrency":"USD","secret":"x"},
            "commerce":{"price":12.5,"taxRate":19,"media":[{"url":"https://cdn.example/a.png","alt":"hidden"}],"stock":7},
            "catalog":{"productNumber":"SKU-1","active":true,"parentId":null,"options":{"size":"M"}},
            "translations":{"en-GB":{"name":"Lamp","description":"Bright","internal":"x"}}});
        let v = projection(&value, 2).unwrap();
        assert_eq!(v["price"], json!({"amount":"12.50","currency":"USD"}));
        assert_eq!(v["sku"], "SKU-1");
        assert_eq!(v["active"], true);
        assert_eq!(v["options"], json!({"size":"M"}));
        assert_eq!(v["taxRate"], 19);
        assert_eq!(v["media"], json!([{"url":"https://cdn.example/a.png"}]));
        assert_eq!(
            v["translations"],
            json!({"en-GB":{"name":"Lamp","description":"Bright"}})
        );
        assert!(v.get("stock").is_none() && v.get("extra").is_none());
        let plain = projection(&json!({"commerce":{"price":3}}), 0).unwrap();
        assert_eq!(plain["price"], json!({"amount":"3","currency":"EUR"}));
        assert_eq!(plain["sku"], "");
    }

    #[test]
    fn text_and_media_are_bounded_and_invalid_money_is_refused() {
        let media = (0..12)
            .map(|i| json!({"url":format!("https://cdn.example/{i}.png")}))
            .collect::<Vec<_>>();
        let v = projection(&json!({"commerce":{"price":1,"media":media},"translations":{"en-GB":{"name":"x".repeat(5000)}}}), 2).unwrap();
        assert_eq!(v["media"].as_array().unwrap().len(), 8);
        assert_eq!(
            v["translations"]["en-GB"]["name"].as_str().unwrap().len(),
            4000
        );
        assert!(projection(&json!({"commerce":{"price":"abc"}}), 2).is_err());
        assert!(projection(&json!({"commerce":{}}), 2).is_err());
        assert!(
            projection(
                &json!({"extra":{"priceCurrency":"!!"},"commerce":{"price":1}}),
                2
            )
            .is_err()
        );
    }
}
