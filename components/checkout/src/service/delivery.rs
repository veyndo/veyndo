//! Shipping costs, proportional taxes and calendar delivery windows.
use crate::domain::quote_view::{DeliveryView, digital, flags};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::document_slot::field;
use veyndo_core::{Error, Result, StatusCode, bad};
use veyndo_pricing::binary::Binary;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::decimal::Decimal;
use veyndo_pricing::money::Minor;
use veyndo_pricing::pricing::{PriceInput, TaxRule, calculate, math_round, proportional_tax_rules};
use veyndo_pricing::service::conversion as currencies;
use veyndo_pricing::types::currency::Definition;
use veyndo_pricing::verified_kernel;
use veyndo_shop::domain::method_text;
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::settings::{CheckoutSelection, Settings, Shipping};

pub use veyndo_shop::types::cart::selection;
/// The parts of a priced quote the delivery cost reads. Totals and line taxes are required:
/// a quote without them is refused, never priced from defaults.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PricedQuote {
    price: QuoteTotals,
    line_items: Vec<PricedLine>,
    #[serde(default, deserialize_with = "lenient")]
    promotion_free_shipping: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    app_shipping_adjustment_minor: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteTotals {
    position_price: Binary,
    total_price: Binary,
    net_price: Binary,
    tax: Binary,
}

#[derive(Deserialize)]
struct PricedLine {
    price: LinePrice,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinePrice {
    calculated_taxes: Vec<StoredTax>,
}

/// One stored line tax as the quote holds it: its exact binary64 numbers.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct StoredTax {
    tax: Binary,
    tax_rate: Binary,
    price: Binary,
}

impl PricedQuote {
    fn read(q: &Value) -> Result<Self> {
        serde_json::from_value(q.clone()).map_err(|_| {
            Error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Priced quote is incomplete".into(),
            )
        })
    }

    /// The basket taxes summed per rate, in the order the rates first appear (binary sums of
    /// the stored numbers, as before).
    fn taxes(&self) -> Result<Vec<StoredTax>> {
        let mut taxes: Vec<StoredTax> = vec![];
        for v in self
            .line_items
            .iter()
            .flat_map(|l| &l.price.calculated_taxes)
        {
            if let Some(t) = taxes.iter_mut().find(|t| t.tax_rate == v.tax_rate) {
                t.price = t.price.checked_add(v.price).map_err(bad)?;
                t.tax = t.tax.checked_add(v.tax).map_err(bad)?;
            } else {
                taxes.push(*v);
            }
        }
        Ok(taxes)
    }
}

/// The tax rules of the delivery cost: the highest rate of the basket or its proportional mix.
fn shipping_tax_rules(
    taxes: &[StoredTax],
    shipping: &Shipping,
    position: Binary,
) -> Result<Vec<TaxRule>> {
    if shipping.tax_type == "highest" {
        Ok(taxes
            .iter()
            .map(|t| t.tax_rate)
            .max()
            .map(|tax_rate| {
                vec![TaxRule {
                    tax_rate,
                    percentage: Binary::integer(100),
                }]
            })
            .unwrap_or_default())
    } else {
        let shares = taxes
            .iter()
            .map(|t| (t.tax_rate, t.price))
            .collect::<Vec<_>>();
        proportional_tax_rules(&shares, position).map_err(bad)
    }
}
/// The method candidates for every country and the methods available in the selected one.
fn method_options(
    q: &mut Value,
    c: &StoredCart,
    s: &Settings,
    selected: &CheckoutSelection,
    view: &dyn Fn(&Shipping) -> Result<Value>,
) -> Result<()> {
    // Candidates support country edits on one page. They are not an admission decision.
    let shipping_method_options = s
        .shipping
        .iter()
        .filter(|v| v.active)
        .map(view)
        .collect::<Result<Vec<_>>>()?;
    let payment_method_options = s
        .payments
        .iter()
        .filter(|v| v.active && (!v.business_only || s.is_business(&c.data.group)))
        .map(|v| {
            let mut method = method_text::localized_payment(v, &c.data.locale, s);
            merge(
                &mut method,
                PaymentCountries {
                    countries: s
                        .countries
                        .iter()
                        .filter(|country| v.available_in(country))
                        .collect(),
                    restricted_countries: true,
                },
            )?;
            Ok(method)
        })
        .collect::<Result<Vec<_>>>()?;
    let available_shipping_methods = s
        .shipping
        .iter()
        .filter(|v| v.active && v.countries.contains(&selected.country))
        .map(view)
        .collect::<Result<Vec<_>>>()?;
    let available_payment_methods = s
        .payments
        .iter()
        .filter(|v| {
            v.active
                && (!v.business_only || s.is_business(&c.data.group))
                && v.available_in(&selected.country)
        })
        .map(|v| method_text::localized_payment(v, &c.data.locale, s))
        .collect();
    merge(
        q,
        MethodOptions {
            shipping_method_options,
            payment_method_options,
            available_shipping_methods,
            available_payment_methods,
        },
    )
}

/// The countries a payment method candidate may be chosen in.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymentCountries<'a> {
    countries: Vec<&'a String>,
    restricted_countries: bool,
}

/// Every active method as a candidate, and the methods available in the selected country.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MethodOptions {
    shipping_method_options: Vec<Value>,
    payment_method_options: Vec<Value>,
    available_shipping_methods: Vec<Value>,
    available_payment_methods: Vec<Value>,
}

/// A shipping method's price and free-shipping threshold in the cart currency.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShippingPrice {
    price: Decimal,
    free_above: Option<Decimal>,
}

/// A shipping method localized for the buyer, with its price and free-shipping threshold in the currency.
pub fn priced_shipping(
    v: &Shipping,
    locale: &str,
    s: &Settings,
    currency: &Definition,
) -> Result<Value> {
    let mut result = method_text::localized_shipping(v, locale, s);
    merge(
        &mut result,
        ShippingPrice {
            price: currencies::convert(currencies::stored(v.price)?, &s.currencies, currency)?,
            free_above: v
                .free_above
                .map(|v| currencies::convert(currencies::stored(v)?, &s.currencies, currency))
                .transpose()?,
        },
    )?;
    Ok(result)
}

/// The quote totals with the delivery cost.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeliveredTotals {
    total_price: Minor,
    net_price: Minor,
    tax: Minor,
}

/// The delivery terms a quote was priced under.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryTerms<'a> {
    shipping_costs: &'a Value,
    checkout: &'a CheckoutSelection,
    configuration_revision: i64,
    available_countries: &'a [String],
}

/// The deliveries of a quote and its localized payment method.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeliveriesAndPayment {
    deliveries: Value,
    payment_method: Value,
}

/// The calendar window of a delivery.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatedDelivery {
    delivery_date: DateWindow,
}

/// The earliest and latest delivery day counted in calendar days.
#[derive(Serialize)]
struct DateWindow {
    earliest: String,
    latest: String,
    basis: &'static str,
}
pub fn enrich(
    mut q: Value,
    c: &StoredCart,
    ps: &[Product],
    s: &Settings,
    revision: i64,
) -> Result<Value> {
    let currency = s.currencies.selected(&c.data.currency)?;
    let decimals = currency.scale;
    let shipping_view =
        |v: &Shipping| -> Result<Value> { priced_shipping(v, &c.data.locale, s, currency) };
    let selected = selection(&c.data);
    let shipping = s
        .shipping
        .iter()
        .find(|v| {
            v.id == selected.shipping_method_id
                && v.active
                && v.countries.contains(&selected.country)
        })
        .ok_or(bad("Shipping method is unavailable for this country"))?;
    let payment = s
        .payments
        .iter()
        .find(|v| {
            v.id == selected.payment_method_id
                && v.active
                && (!v.business_only || s.is_business(&c.data.group))
                && v.available_in(&selected.country)
        })
        .ok_or(bad("Payment method is unavailable for this customer"))?;
    let priced = PricedQuote::read(&q)?;
    let position = priced.price.position_price;
    let gross_items = priced.price.total_price;
    let free = shipping
        .free_above
        .map(|limit| {
            let limit = currencies::convert(currencies::stored(limit)?, &s.currencies, currency)?;
            Binary::from_decimal(limit).map_err(bad)
        })
        .transpose()?
        .is_some_and(|limit| gross_items >= limit);
    let all_free = !c.data.items.is_empty()
        && c.data.items.iter().all(|i| {
            ps.iter()
                .find(|p| p.id == i.id)
                .is_some_and(|p| flags(&p.extra).shipping_free == true || digital(&p.extra))
        });
    let base = if c.data.items.is_empty()
        || free
        || all_free
        || priced.promotion_free_shipping == Some(true)
    {
        Binary::ZERO
    } else {
        let price =
            currencies::convert(currencies::stored(shipping.price)?, &s.currencies, currency)?;
        Binary::from_decimal(price).map_err(bad)?
    };
    // The method price plus the app adjustment, rounded once to the minor unit as before.
    let adjustment = Minor::new(priced.app_shipping_adjustment_minor.unwrap_or(0), decimals);
    let gross = base
        .checked_add(Binary::from_decimal(adjustment.decimal()).map_err(bad)?)
        .and_then(|g| math_round(g, decimals))
        .map_err(bad)?;
    if gross.minor() < 0 {
        return Err(bad(
            "App shipping adjustment cannot make delivery cost negative",
        ));
    }
    let rules = shipping_tax_rules(&priced.taxes()?, shipping, position)?;
    let gross_calc = calculate(&PriceInput {
        price: Binary::from_decimal(gross.decimal()).map_err(bad)?,
        decimals: decimals.into(),
        interval: Binary::from_decimal(Decimal::new(1, decimals.into()).map_err(bad)?)
            .map_err(bad)?,
        quantity: 1,
        tax_rules: Some(rules.clone()),
        ..PriceInput::default()
    })
    .map_err(bad)?;
    // Whole minor units from here on: net shipping and the delivered totals use the proved rules.
    let unsigned = |amount: Minor| -> Result<u64> {
        amount
            .at_scale(decimals)
            .and_then(Minor::unsigned)
            .map_err(bad)
    };
    let stored =
        |amount: Binary| -> Result<u64> { unsigned(math_round(amount, decimals).map_err(bad)?) };
    let amount = |minor: u64| Minor::from_unsigned(minor, decimals).map_err(bad);
    let shipping_tax = unsigned(gross_calc.tax)?;
    let shipping_net = amount(verified_kernel::net_amount(unsigned(gross)?, shipping_tax))?;
    // Native method prices are gross amounts; explicit net component for B2B display.
    let cost = json!({"unitPrice":if s.is_business(&c.data.group){shipping_net}else{gross},"totalPrice":gross,"netPrice":shipping_net,"tax":gross_calc.tax,"calculatedTaxes":gross_calc.calculated_taxes});
    merge(
        field(&mut q, "price")?,
        DeliveredTotals {
            total_price: amount(verified_kernel::amount_sum(
                stored(gross_items)?,
                unsigned(gross)?,
            ))?,
            net_price: amount(verified_kernel::amount_sum(
                stored(priced.price.net_price)?,
                unsigned(shipping_net)?,
            ))?,
            tax: amount(verified_kernel::amount_sum(
                stored(priced.price.tax)?,
                shipping_tax,
            ))?,
        },
    )?;
    merge(
        &mut q,
        DeliveryTerms {
            shipping_costs: &cost,
            checkout: &selected,
            configuration_revision: revision,
            available_countries: &s.countries,
        },
    )?;
    method_options(&mut q, c, s, &selected, &shipping_view)?;
    let lead = c
        .data
        .items
        .iter()
        .filter_map(|i| ps.iter().find(|p| p.id == i.id))
        .map(|p| p.delivery_days)
        .max()
        .unwrap_or(0);
    let (min, max) = if shipping.id == "pickup" {
        (0, 0)
    } else {
        (lead.max(shipping.min_days), lead.max(shipping.max_days))
    };
    let deliveries = if c.data.items.is_empty()
        || c.data
            .items
            .iter()
            .all(|i| ps.iter().any(|p| p.id == i.id && digital(&p.extra)))
    {
        json!([])
    } else {
        json!([{"shippingMethod":shipping_view(shipping)?,"shippingCosts":cost,"shippingLocation":{"country":selected.country,"address":selected.address},"positions":c.data.items.iter().filter(|i|!ps.iter().any(|p|p.id==i.id&&digital(&p.extra))).collect::<Vec<_>>(),"state":"open","deliveryTime":{"minDays":min,"maxDays":max}}])
    };
    merge(
        &mut q,
        DeliveriesAndPayment {
            deliveries,
            payment_method: method_text::localized_payment(payment, &c.data.locale, s),
        },
    )?;
    Ok(q)
}
pub async fn dates_conn(conn: &mut sqlx::PgConnection, q: &mut Value) -> Result<()> {
    if let Some(d) = field(q, "deliveries")?
        .as_array_mut()
        .and_then(|d| d.first_mut())
    {
        let time = view::<DeliveryView>(d).delivery_time;
        let min = time.min_days.as_i64().unwrap() as i32;
        let max = time.max_days.as_i64().unwrap() as i32;
        let (earliest, latest) =
            veyndo_shipping::store::delivery_dates::window(conn, min, max).await?;
        merge(
            d,
            DatedDelivery {
                delivery_date: DateWindow {
                    earliest,
                    latest,
                    basis: "calendar-days",
                },
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::PricedQuote;
    use serde_json::{Value, json};
    use veyndo_pricing::binary::Binary;

    /// The previous untyped reading, kept as the oracle of the typed one: (rate, tax, price) per rate.
    #[allow(clippy::type_complexity)]
    fn legacy(q: &Value) -> (f64, f64, f64, f64, Vec<(f64, f64, f64)>, bool, i64) {
        let mut taxes: Vec<(f64, f64, f64)> = vec![];
        for line in q["lineItems"].as_array().unwrap() {
            for v in line["price"]["calculatedTaxes"].as_array().unwrap() {
                let rate = v["taxRate"].as_f64().unwrap();
                if let Some(t) = taxes.iter_mut().find(|t| t.0 == rate) {
                    t.2 += v["price"].as_f64().unwrap();
                    t.1 += v["tax"].as_f64().unwrap();
                } else {
                    taxes.push((
                        rate,
                        v["tax"].as_f64().unwrap(),
                        v["price"].as_f64().unwrap(),
                    ));
                }
            }
        }
        (
            q["price"]["positionPrice"].as_f64().unwrap(),
            q["price"]["totalPrice"].as_f64().unwrap(),
            q["price"]["netPrice"].as_f64().unwrap(),
            q["price"]["tax"].as_f64().unwrap(),
            taxes,
            q["promotionFreeShipping"] == true,
            q["appShippingAdjustmentMinor"].as_i64().unwrap_or(0),
        )
    }

    #[test]
    fn the_typed_quote_reads_what_the_untyped_reader_read_and_refuses_what_it_panicked_on() {
        let line = |rate: f64, tax: f64, price: f64| json!({"id":"l","quantity":1,"price":{"unitPrice":price,"totalPrice":price,"calculatedTaxes":[{"taxRate":rate,"tax":tax,"price":price}]}});
        for extra in [
            json!({}),
            json!({"promotionFreeShipping":true,"appShippingAdjustmentMinor":-150}),
            json!({"promotionFreeShipping":"true","appShippingAdjustmentMinor":1.5}),
            json!({"promotionFreeShipping":[true],"appShippingAdjustmentMinor":"9"}),
        ] {
            let mut q = json!({"price":{"positionPrice":30,"totalPrice":30.0,"netPrice":25.5,"tax":4.5,"currency":"EUR"},
                "lineItems":[line(19., 1.6, 10.), line(7., 0.65, 10.), line(19., 1.6, 10.), {"price":{"calculatedTaxes":[]}}]});
            q.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let typed = PricedQuote::read(&q).unwrap();
            let wire = Binary::to_wire;
            let typed = (
                wire(&typed.price.position_price),
                wire(&typed.price.total_price),
                wire(&typed.price.net_price),
                wire(&typed.price.tax),
                typed
                    .taxes()
                    .unwrap()
                    .iter()
                    .map(|t| (wire(&t.tax_rate), wire(&t.tax), wire(&t.price)))
                    .collect::<Vec<_>>(),
                typed.promotion_free_shipping == Some(true),
                typed.app_shipping_adjustment_minor.unwrap_or(0),
            );
            assert_eq!(typed, legacy(&q), "{q}");
        }
        for broken in [
            json!({"price":{"positionPrice":"30","totalPrice":30,"netPrice":25,"tax":5},"lineItems":[]}),
            json!({"price":{"positionPrice":30,"totalPrice":30,"netPrice":25,"tax":5}}),
            json!({"price":{"positionPrice":30,"totalPrice":30,"netPrice":25,"tax":5},"lineItems":[{"price":{"calculatedTaxes":[{"taxRate":19,"tax":null,"price":1}]}}]}),
        ] {
            assert!(PricedQuote::read(&broken).is_err());
        }
    }

    /// The typed quote patches write exactly what the indexed assignments wrote.
    #[test]
    fn typed_quote_patches_match_the_indexed_writes() {
        use super::{
            DateWindow, DatedDelivery, DeliveredTotals, DeliveriesAndPayment, MethodOptions, Minor,
            PaymentCountries, ShippingPrice,
        };
        use veyndo_core::document::merge;
        use veyndo_core::document_slot::field;
        let base = json!({"id":"cart","price":{"totalPrice":10.0,"positionPrice":10.0},"deliveries":[{"deliveryTime":{"minDays":1,"maxDays":2}}]});
        let mut indexed = base.clone();
        indexed["price"]["totalPrice"] = json!(14.9);
        indexed["price"]["netPrice"] = json!(12.52);
        indexed["price"]["tax"] = json!(2.38);
        indexed["shippingMethodOptions"] = json!([{"id":"s"}]);
        indexed["paymentMethodOptions"] =
            json!([{"id":"p","countries":["DE"],"restrictedCountries":true}]);
        indexed["availableShippingMethods"] = json!([]);
        indexed["availablePaymentMethods"] = json!([{"id":"p"}]);
        indexed["deliveries"] = json!([{"state":"open"}]);
        indexed["paymentMethod"] = json!({"id":"p"});
        indexed["deliveries"][0]["deliveryDate"] =
            json!({"earliest":"2026-01-01","latest":"2026-01-03","basis":"calendar-days"});
        let mut typed = base.clone();
        merge(
            field(&mut typed, "price").unwrap(),
            DeliveredTotals {
                total_price: Minor::new(1490, 2),
                net_price: Minor::new(1252, 2),
                tax: Minor::new(238, 2),
            },
        )
        .unwrap();
        let de = "DE".to_string();
        let mut method = json!({"id":"p"});
        merge(
            &mut method,
            PaymentCountries {
                countries: vec![&de],
                restricted_countries: true,
            },
        )
        .unwrap();
        merge(
            &mut typed,
            MethodOptions {
                shipping_method_options: vec![json!({"id":"s"})],
                payment_method_options: vec![method],
                available_shipping_methods: vec![],
                available_payment_methods: vec![json!({"id":"p"})],
            },
        )
        .unwrap();
        merge(
            &mut typed,
            DeliveriesAndPayment {
                deliveries: json!([{"state":"open"}]),
                payment_method: json!({"id":"p"}),
            },
        )
        .unwrap();
        let first = field(&mut typed, "deliveries")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .first_mut()
            .unwrap();
        merge(
            first,
            DatedDelivery {
                delivery_date: DateWindow {
                    earliest: "2026-01-01".into(),
                    latest: "2026-01-03".into(),
                    basis: "calendar-days",
                },
            },
        )
        .unwrap();
        assert_eq!(typed, indexed);
        let mut shipping = json!({"id":"s","name":"Post"});
        let mut indexed = shipping.clone();
        indexed["price"] = json!(4.9);
        indexed["freeAbove"] = json!(Option::<f64>::None);
        merge(
            &mut shipping,
            ShippingPrice {
                price: veyndo_pricing::decimal::Decimal::parse("4.9").unwrap(),
                free_above: None,
            },
        )
        .unwrap();
        assert_eq!(shipping, indexed);
    }
}
