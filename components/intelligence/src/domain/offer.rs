//! The positive projection of a native quote that the shop signs as an offer: a statement, not a price lock.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad, conflict};

const VALID_SECONDS: i64 = 600;
/// The quote fields an offer signs; everything else (buyer data, the cart capability) is never read.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OfferQuote {
    id: Value,
    revision: Value,
    configuration_revision: Value,
    #[serde(deserialize_with = "lenient")]
    price: OfferPrice,
    #[serde(deserialize_with = "lenient")]
    shipping_costs: PriceOnly,
    #[serde(deserialize_with = "lenient")]
    checkout: OfferSelection,
    #[serde(deserialize_with = "lenient")]
    line_items: Option<Vec<Value>>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OfferPrice {
    currency: Value,
    currency_scale: Value,
    #[serde(deserialize_with = "lenient")]
    total_minor: Option<String>,
    total_price: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PriceOnly {
    total_price: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OfferSelection {
    country: Value,
    shipping_method_id: Value,
    payment_method_id: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct OfferLine {
    id: Value,
    label: Value,
    quantity: Value,
    #[serde(deserialize_with = "lenient")]
    price: LinePrice,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LinePrice {
    unit_price: Value,
    total_price: Value,
}

/// The positive projection that is signed: totals, lines and the reviewed selection, never buyer data or the cart capability.
pub fn payload(t: &str, channel: &str, q: &Value, now: i64) -> Result<Value> {
    let q: OfferQuote = view(q);
    let lines = q
        .line_items
        .filter(|lines| !lines.is_empty())
        .ok_or(conflict("An empty cart has no offer"))?;
    let total = q
        .price
        .total_minor
        .ok_or(bad("Native quote has no exact total"))?;
    Ok(
        json!({"schemaVersion":1,"kind":"offer","tenant":t,"channel":channel,
        "cartId":q.id,"cartRevision":q.revision,"configurationRevision":q.configuration_revision,
        "currency":q.price.currency,"currencyScale":q.price.currency_scale,"totalMinor":total,"totalPrice":q.price.total_price,
        "shippingPrice":q.shipping_costs.total_price,
        "country":q.checkout.country,"shippingMethodId":q.checkout.shipping_method_id,"paymentMethodId":q.checkout.payment_method_id,
        "lineItems":lines.iter().map(view::<OfferLine>).map(|i|json!({"id":i.id,"label":i.label,"quantity":i.quantity,"unitPrice":i.price.unit_price,"totalPrice":i.price.total_price})).collect::<Vec<_>>(),
        "issuedAt":now,"expiresAt":now+VALID_SECONDS,
        "meaning":"The shop's calculated quote for this cart revision at issue time. Checkout confirms exactly this revision and total, or refuses and a new offer is needed. It reserves no stock and does not lock a price the merchant changes afterwards."}),
    )
}
#[cfg(test)]
mod tests {
    use super::payload;
    use serde_json::json;
    #[test]
    fn signs_only_the_closed_projection_and_refuses_an_empty_cart() {
        let quote = json!({"id":"c1","revision":4,"configurationRevision":9,"token":"secret-capability","customerEmail":"buyer@example.test",
            "checkout":{"country":"DE","shippingMethodId":"standard","paymentMethodId":"demo-card","address":{"street":"Private 1"}},
            "price":{"currency":"EUR","currencyScale":2,"totalMinor":"12290","totalPrice":122.9},"shippingCosts":{"totalPrice":4.9},
            "paymentMethod":{"id":"demo-card","name":"Demo"},
            "lineItems":[{"id":"scarf","label":"Scarf","quantity":2,"stock":30,"price":{"unitPrice":59.0,"totalPrice":118.0}}]});
        let offer = payload("shop", "default", &quote, 1000).unwrap();
        let text = offer.to_string();
        assert!(
            !text.contains("secret-capability")
                && !text.contains("buyer@example.test")
                && !text.contains("Private 1")
        );
        assert!(offer["lineItems"][0].get("stock").is_none());
        assert_eq!(offer["totalMinor"], "12290");
        assert_eq!(offer["cartRevision"], 4);
        assert_eq!(offer["currency"], "EUR");
        assert_eq!(offer["channel"], "default");
        assert_eq!(offer["paymentMethodId"], "demo-card");
        assert_eq!(offer["expiresAt"], 1600);
        assert_eq!(
            offer["lineItems"][0],
            json!({"id":"scarf","label":"Scarf","quantity":2,"unitPrice":59.0,"totalPrice":118.0})
        );
        let mut empty = quote.clone();
        empty["lineItems"] = json!([]);
        assert!(payload("shop", "default", &empty, 1000).is_err());
    }
}
