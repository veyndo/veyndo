//! Standard order read fields are projected from the authoritative quote/payment, never maintained twice.
use crate::types::order_document::{OrderDocument, QuotePrice, Totalled};
use serde::Serialize;
use serde_json::{Map, Value, json};
use veyndo_core::document::view;

/// The read fields an order exposes, derived from its stored quote and payment.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    state_labels: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state_main_locale: Option<Value>,
    amount_total: Value,
    amount_net: Value,
    shipping_total: Value,
    price: Value,
    currency: Value,
    currency_scale: Value,
    total_minor: Value,
    line_items: Value,
    shipping_costs: Value,
    currency_id: Value,
    currency_factor: Value,
    tax_status: Value,
    transactions: [Transaction; 1],
    addresses: Vec<Value>,
}

/// The single payment transaction of the order.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Transaction {
    id: String,
    payment_method_id: Value,
    state: Value,
    amount: Value,
    provider: Value,
    real_money_charged: Value,
}

/// Adds the derived read fields to a stored order document; anything but an object is left alone.
pub fn order_fields(o: &mut Value) {
    if !o.is_object() {
        return;
    }
    let labels = veyndo_app_model::native_lifecycle_snapshot::state_labels(o);
    let d: OrderDocument = view(o);
    let price: QuotePrice = view(&d.cart.price);
    let shipping: Totalled = view(&d.cart.shipping_costs);
    // Exact-money API consumers use the immutable quote, including its actual currency scale.
    let factor = d
        .cart
        .currency_context
        .factor
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.);
    let factor = json!(factor);
    let mut addresses = Vec::new();
    for (address, id) in [
        (&d.billing_address, &d.billing_address_id),
        (&d.shipping_address, &d.shipping_address_id),
    ] {
        if let Value::Object(fields) = address {
            let mut a: Map<String, Value> = fields.clone();
            a.insert("id".into(), id.clone());
            addresses.push(Value::Object(a));
        }
    }
    let fields = ReadFields {
        state_main_locale: labels.as_ref().map(|(_, main)| main.clone()),
        state_labels: labels.map(|(labels, _)| labels),
        amount_total: price.total_price,
        amount_net: price.net_price,
        shipping_total: shipping.total_price,
        currency: d.money.currency.code,
        currency_scale: d.money.currency.scale,
        total_minor: d.money.minor,
        line_items: d.cart.line_items,
        shipping_costs: d.cart.shipping_costs,
        currency_id: json!(price.currency.as_str().unwrap_or("EUR")),
        currency_factor: if factor.is_null() { json!(1) } else { factor },
        tax_status: price.tax_status,
        transactions: [Transaction {
            id: d
                .payment
                .attempt_id
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{}-payment", d.id.as_str().unwrap_or(""))),
            payment_method_id: d.payment.method.id,
            state: d.payment.state,
            amount: d.cart.price.clone(),
            provider: d.payment.provider,
            real_money_charged: d.payment.real_money_charged,
        }],
        price: d.cart.price,
        addresses,
    };
    if let (Value::Object(target), Ok(Value::Object(derived))) = (o, serde_json::to_value(fields)) {
        target.extend(derived);
    }
}
#[cfg(test)]
mod tests {
    use super::order_fields;
    use serde_json::{Value, json};

    /// The untyped projection this module replaced; the typed one must produce the same document.
    mod legacy {
        use serde_json::{Value, json};
        pub(super) fn order_fields(o: &mut Value) {
            if !o.is_object() {
                return;
            }
            if let Some((labels, main)) =
                veyndo_app_model::native_lifecycle_snapshot::state_labels(o)
            {
                o["stateLabels"] = labels;
                o["stateMainLocale"] = main;
            }
            o["amountTotal"] = o["cart"]["price"]["totalPrice"].clone();
            o["amountNet"] = o["cart"]["price"]["netPrice"].clone();
            o["shippingTotal"] = o["cart"]["shippingCosts"]["totalPrice"].clone();
            o["price"] = o["cart"]["price"].clone();
            // Exact-money API consumers use the immutable quote, including its actual currency scale.
            o["currency"] = o["money"]["currency"]["code"].clone();
            o["currencyScale"] = o["money"]["currency"]["scale"].clone();
            o["totalMinor"] = o["money"]["minor"].clone();
            o["lineItems"] = o["cart"]["lineItems"].clone();
            o["shippingCosts"] = o["cart"]["shippingCosts"].clone();
            o["currencyId"] = o["cart"]["price"]["currency"]
                .as_str()
                .map(|c| json!(c))
                .unwrap_or(json!("EUR"));
            o["currencyFactor"] = json!(
                o["cart"]["currencyContext"]["factor"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.)
            );
            if o["currencyFactor"].is_null() {
                o["currencyFactor"] = json!(1);
            }
            o["taxStatus"] = o["cart"]["price"]["taxStatus"].clone();
            o["transactions"] = json!([{"id":o["payment"]["attemptId"].as_str().map(str::to_owned).unwrap_or_else(||format!("{}-payment",o["id"].as_str().unwrap_or(""))),"paymentMethodId":o["payment"]["method"]["id"],"state":o["payment"]["state"],"amount":o["cart"]["price"],"provider":o["payment"]["provider"],"realMoneyCharged":o["payment"]["realMoneyCharged"]}]);
            let mut addresses = Vec::new();
            for (kind, id) in [
                ("billingAddress", "billingAddressId"),
                ("shippingAddress", "shippingAddressId"),
            ] {
                if o[kind].is_object() {
                    let mut a = o[kind].clone();
                    a["id"] = o[id].clone();
                    addresses.push(a);
                }
            }
            o["addresses"] = json!(addresses);
        }
    }

    #[test]
    fn payment_projection_follows_provider_state() {
        let mut o = json!({"id":"order","cart":{"price":{"totalPrice":19.99,"netPrice":16.8,"taxStatus":"gross"},"shippingCosts":{"totalPrice":0},"lineItems":[]},"payment":{"state":"pending","method":{"id":"paypal-live"},"provider":"paypal","realMoneyCharged":false}});
        order_fields(&mut o);
        assert_eq!(o["transactions"][0]["state"], "pending");
        o["payment"]["state"] = json!("captured");
        o["payment"]["realMoneyCharged"] = json!(true);
        order_fields(&mut o);
        assert_eq!(o["transactions"][0]["state"], "captured");
        assert_eq!(o["transactions"][0]["realMoneyCharged"], true);
        assert_eq!(o["amountTotal"], 19.99);
    }
    #[test]
    fn exact_api_amount_comes_from_the_order_snapshot_not_recomputed_float_price() {
        let mut o = json!({"cart":{"price":{"totalPrice":99.99,"currency":"EUR"}},"money":{"minor":1200,"currency":{"code":"JPY","scale":0}}});
        order_fields(&mut o);
        assert_eq!(o["totalMinor"], 1200);
        assert_eq!(o["currency"], "JPY");
        assert_eq!(o["currencyScale"], 0);
    }

    #[test]
    fn typed_fields_equal_the_untyped_projection() {
        let cases = [
            json!({"id":"o1","cart":{"price":{"totalPrice":11.9,"netPrice":10,"taxStatus":"gross","currency":"USD"},"shippingCosts":{"totalPrice":4.5},"lineItems":[{"id":"l"}],"currencyContext":{"factor":"1.25"}},
                "money":{"minor":1190,"currency":{"code":"USD","scale":2}},"payment":{"attemptId":"a1","method":{"id":"m"},"state":"captured","provider":"p","realMoneyCharged":true},
                "billingAddress":{"name":"Ada"},"billingAddressId":"b1","shippingAddress":"x","shippingAddressId":"s1"}),
            json!({"id":"o2","cart":null,"money":"x","payment":null,"billingAddress":{"name":"Ada"}}),
            json!({"cart":{"currencyContext":{"factor":"NaN"}}}),
            json!({"cart":{"currencyContext":{"factor":"inf"},"price":"flat"}}),
            json!({"id":"o3","cart":[1,2],"money":[5,"EUR"],"payment":["x"],"billingAddress":["Ada"]}),
            json!({"cart":{"price":[19.99,16.8],"currencyContext":["1.25"]}}),
            json!({}),
        ];
        for case in cases {
            let (mut typed, mut untyped) = (case.clone(), case.clone());
            order_fields(&mut typed);
            legacy::order_fields(&mut untyped);
            assert_eq!(typed, untyped, "{case}");
        }
        let mut text = json!("not an object");
        order_fields(&mut text);
        assert_eq!(text, json!("not an object"));
        let _: Value = text;
    }
}
