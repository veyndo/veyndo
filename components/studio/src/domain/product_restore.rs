//! Rebuilds a product edit from a recorded version; stock stays current and associations pass the normal product admission afterwards.
use serde::Deserialize;
use serde_json::{Map, Value, json};
use veyndo_core::Result;
use veyndo_core::document::{lenient, lenient_list, merge, view};

/// The recorded product columns a restoration reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Recorded {
    price: Value,
    tax_rate: Value,
    min_purchase: Value,
    purchase_steps: Value,
    max_purchase: Value,
    delivery_days: Value,
    list_price: Value,
    regulation_price: Value,
    reference_price: Value,
    advanced_prices: Value,
    media: Value,
    properties: Value,
    extra: Value,
    name: Value,
    description: Value,
    active: Value,
    product_number: Value,
    parent_id: Value,
    options: Value,
}

impl Recorded {
    /// The commerce fields of the editor and the recorded columns they are restored from.
    fn commerce(self) -> [(&'static str, Value); 12] {
        [
            ("price", self.price),
            ("taxRate", self.tax_rate),
            ("minPurchase", self.min_purchase),
            ("purchaseSteps", self.purchase_steps),
            ("maxPurchase", self.max_purchase),
            ("deliveryDays", self.delivery_days),
            ("listPrice", self.list_price),
            ("regulationPrice", self.regulation_price),
            ("referencePrice", self.reference_price),
            ("advancedPrices", self.advanced_prices),
            ("media", self.media),
            ("properties", self.properties),
        ]
    }
}

/// A channel's recorded or current visibility.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Channel {
    id: Value,
    visible: Value,
}

/// A recorded product version: its row, translations by locale, categories and channels.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Version {
    #[serde(deserialize_with = "lenient")]
    record: Recorded,
    #[serde(deserialize_with = "lenient")]
    translations: Map<String, Value>,
    categories: Value,
    #[serde(deserialize_with = "lenient_list")]
    channels: Vec<Channel>,
}

/// The channels of the current editor document.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Editor {
    #[serde(deserialize_with = "lenient_list")]
    channels: Vec<Channel>,
}

/// A translation's name, the only field a restoration inspects.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Translation {
    name: Value,
}

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
pub fn edit(
    current: &mut Value,
    state: &Value,
    id: &str,
    locales: &[String],
    main: &str,
) -> Result<()> {
    let version: Version = view(state);
    let editor: Editor = view(current);
    let r = version.record;
    let mut catalog = json!({"active":r.active,"productNumber":r.product_number.as_str().unwrap_or(id),"parentId":r.parent_id,"options":r.options,"categoryIds":version.categories});
    let main_fallback = json!({"name":r.name,"description":r.description});
    let extra = r.extra.clone();
    let mut commerce = current.get("commerce").cloned().unwrap_or(Value::Null);
    merge(
        &mut commerce,
        r.commerce()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<Map<_, _>>(),
    )?;
    if let Some(commerce) = commerce.as_object_mut() {
        commerce.remove("stock");
    }
    let mut tr = Map::new();
    for locale in locales {
        let key = editor_key(locale, locales);
        let mut value = version
            .translations
            .get(locale)
            .cloned()
            .unwrap_or(Value::Null);
        if value.is_null() {
            value = json!({"name":null,"description":null});
        }
        if locale == main && view::<Translation>(&value).name.is_null() {
            value = main_fallback.clone();
        }
        tr.insert(key.to_owned(), value);
    }
    let hidden = |channel: &Channel| {
        version
            .channels
            .iter()
            .any(|v| v.id == channel.id && v.visible == false)
    };
    let channels = editor
        .channels
        .iter()
        .filter(|channel| !hidden(channel))
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    merge(&mut catalog, json!({ "salesChannelIds": channels }))?;
    merge(
        current,
        json!({"extra": extra, "commerce": commerce, "translations": tr, "catalog": catalog}),
    )
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
        edit(&mut current, &state, "lamp-1", &locales, "de-DE").unwrap();
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
        )
        .unwrap();
        assert_eq!(
            inherited["translations"]["fr"],
            json!({"name":null,"description":null})
        );
    }
}
