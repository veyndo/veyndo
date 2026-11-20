//! Rebuilds a product edit from a recorded version; stock stays current and associations pass the normal product admission afterwards.
use serde_json::{Value, json};

/// The commerce fields of the editor and the recorded columns they are restored from.
const FIELDS: [(&str, &str); 12] = [
    ("price", "price"),
    ("taxRate", "tax_rate"),
    ("minPurchase", "min_purchase"),
    ("purchaseSteps", "purchase_steps"),
    ("maxPurchase", "max_purchase"),
    ("deliveryDays", "delivery_days"),
    ("listPrice", "list_price"),
    ("regulationPrice", "regulation_price"),
    ("referencePrice", "reference_price"),
    ("advancedPrices", "advanced_prices"),
    ("media", "media"),
    ("properties", "properties"),
];

/// The key the editor uses for a content language: the bare language while it is unambiguous in the shop.
fn editor_key<'a>(locale: &'a str, locales: &[String]) -> &'a str {
    let base = locale.split('-').next().unwrap_or(locale);
    if locales
        .iter()
        .filter(|l| l.split('-').next() == Some(base))
        .count()
        == 1
    {
        base
    } else {
        locale
    }
}

/// Writes a recorded version over the current editor document. The stock quantity is removed, so a restoration never replays one.
pub fn edit(current: &mut Value, state: &Value, id: &str, locales: &[String], main: &str) {
    let r = &state["record"];
    for (key, column) in FIELDS {
        current["commerce"][key] = r[column].clone();
    }
    if let Some(commerce) = current["commerce"].as_object_mut() {
        commerce.remove("stock");
    }
    current["extra"] = r["extra"].clone();
    let mut tr = json!({});
    for locale in locales {
        let key = editor_key(locale, locales);
        tr[key] = state["translations"][locale].clone();
        if tr[key].is_null() {
            tr[key] = json!({"name":null,"description":null});
        }
        if locale == main && tr[key]["name"].is_null() {
            tr[key] = json!({"name":r["name"],"description":r["description"]});
        }
    }
    current["translations"] = tr;
    let hidden = |channel: &Value| {
        state["channels"].as_array().is_some_and(|old| {
            old.iter()
                .any(|v| v["id"] == channel["id"] && v["visible"] == false)
        })
    };
    let channels = current["channels"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|channel| !hidden(channel))
                .map(|c| c["id"].clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    current["catalog"] = json!({"active":r["active"],"productNumber":r["product_number"].as_str().unwrap_or(id),"parentId":r["parent_id"],"options":r["options"],"categoryIds":state["categories"],"salesChannelIds":channels});
}

#[cfg(test)]
mod tests {
    use super::{edit, editor_key};
    use serde_json::json;

    #[test]
    fn a_language_keeps_its_bare_key_while_it_is_unambiguous() {
        let locales = [
            "de-DE".to_string(),
            "en-GB".to_string(),
            "en-US".to_string(),
        ];
        assert_eq!(editor_key("de-DE", &locales), "de");
        assert_eq!(editor_key("en-GB", &locales), "en-GB");
        assert_eq!(editor_key("en-US", &locales), "en-US");
    }

    #[test]
    fn a_restored_edit_keeps_current_stock_and_follows_the_main_language() {
        let mut current = json!({
            "revision":7,
            "commerce":{"price":10,"stock":3},
            "channels":[{"id":"web"},{"id":"pos"}],
        });
        let state = json!({
            "record":{"price":25,"tax_rate":19,"name":"Lamp","description":"Warm light","active":true,"extra":{"note":"x"},"options":[],"parent_id":null},
            "translations":{"en-GB":{"name":"Lamp","description":"Warm light"}},
            "categories":["lighting"],
            "channels":[{"id":"pos","visible":false},{"id":"web","visible":true}],
        });
        let locales = ["de-DE".to_string(), "en-GB".to_string()];
        edit(&mut current, &state, "lamp-1", &locales, "de-DE");
        assert_eq!(current["commerce"]["price"], 25);
        assert_eq!(current["commerce"]["taxRate"], 19);
        assert!(current["commerce"].get("stock").is_none());
        assert_eq!(current["extra"], json!({"note":"x"}));
        assert_eq!(
            current["translations"]["de"],
            json!({"name":"Lamp","description":"Warm light"})
        );
        assert_eq!(current["translations"]["en"]["name"], "Lamp");
        assert_eq!(current["catalog"]["productNumber"], "lamp-1");
        assert_eq!(current["catalog"]["categoryIds"], json!(["lighting"]));
        assert_eq!(current["catalog"]["salesChannelIds"], json!(["web"]));
        let mut inherited = json!({"commerce":{},"channels":[]});
        edit(
            &mut inherited,
            &state,
            "lamp-1",
            &["fr-FR".to_string()],
            "de-DE",
        );
        assert_eq!(
            inherited["translations"]["fr"],
            json!({"name":null,"description":null})
        );
    }
}
