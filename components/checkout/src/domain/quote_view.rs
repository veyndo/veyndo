//! Typed read views of the cart quote, the placed order and product extras that checkout reads back; every field keeps the JSON value indexing returned.
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};

/// The quote fields checkout reads after pricing.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct QuoteView {
    pub payment_method: Value,
    #[serde(deserialize_with = "lenient")]
    pub price: PriceView,
    pub deliveries: Value,
    pub line_items: Value,
    pub selection_needs_confirmation: Value,
    pub currency_context: Value,
}

/// The total of a quote price; the stored total is the decimal the quote calculated.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PriceView {
    pub total_price: Value,
}

/// How a quote's payment method is settled.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PaymentMethodView {
    pub mode: Value,
    pub provider: Value,
}

/// One quoted line item's price and discount.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct LineView {
    pub id: Value,
    pub price: Value,
    pub discount_percent: Value,
}

/// The delivery time window of a quoted delivery.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct DeliveryView {
    #[serde(deserialize_with = "lenient")]
    pub delivery_time: DeliveryTime,
}

/// Lead days of a delivery.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct DeliveryTime {
    pub min_days: Value,
    pub max_days: Value,
}

/// The product extras checkout decides shipping, legal and tax by.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ProductFlags {
    pub digital: Value,
    pub shipping_free: Value,
    pub tax_class_id: Value,
}

/// A placed order as checkout reads it back: its id, quote and payment.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct OrderRef {
    pub id: Value,
    pub cart: Value,
    pub payment: Value,
}

impl QuoteView {
    /// The typed payment method.
    pub fn method(&self) -> PaymentMethodView {
        view(&self.payment_method)
    }

    /// The first line item; a missing one reads as all `null`.
    pub fn first_line(&self) -> LineView {
        self.line_items.get(0).map(view).unwrap_or_default()
    }
}

/// The typed view of a quote.
pub fn quote_view(q: &Value) -> QuoteView {
    view(q)
}

/// The typed view of a product's extras.
pub fn flags(extra: &Value) -> ProductFlags {
    view(extra)
}

/// The product is digital (no shipment).
pub fn digital(extra: &Value) -> bool {
    flags(extra).digital == true
}

#[cfg(test)]
mod tests {
    use super::{flags, quote_view};
    use serde_json::json;

    #[test]
    fn the_typed_quote_reads_exactly_what_indexing_read() {
        for q in [
            json!({"paymentMethod":{"mode":"app","provider":"paypal"},"price":{"totalPrice":12.5},"deliveries":[{"deliveryTime":{"minDays":1,"maxDays":3}}],"lineItems":[{"id":"p","price":{"unitPrice":1},"discountPercent":10}],"selectionNeedsConfirmation":true,"currencyContext":{"factor":1}}),
            json!({"paymentMethod":"cash","price":null,"lineItems":{}}),
            json!(null),
        ] {
            let v = quote_view(&q);
            assert_eq!(v.method().mode, q["paymentMethod"]["mode"]);
            assert_eq!(v.method().provider, q["paymentMethod"]["provider"]);
            assert_eq!(v.price.total_price, q["price"]["totalPrice"]);
            assert_eq!(v.deliveries, q["deliveries"]);
            assert_eq!(v.first_line().price, q["lineItems"][0]["price"]);
            assert_eq!(
                v.first_line().discount_percent,
                q["lineItems"][0]["discountPercent"]
            );
            assert_eq!(
                v.selection_needs_confirmation,
                q["selectionNeedsConfirmation"]
            );
            assert_eq!(v.currency_context, q["currencyContext"]);
        }
        for extra in [
            json!({"digital":true,"shippingFree":false,"taxClassId":"reduced"}),
            json!(null),
            json!({"digital":"yes"}),
        ] {
            let f = flags(&extra);
            assert_eq!(f.digital, extra["digital"]);
            assert_eq!(f.shipping_free, extra["shippingFree"]);
            assert_eq!(f.tax_class_id, extra["taxClassId"]);
        }
    }
}
