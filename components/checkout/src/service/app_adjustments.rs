//! Apply admitted integer app adjustments through native pricing/tax calculation; extensions cannot replace quote JSON.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Error, Result, StatusCode, bad};
use veyndo_pricing::pricing::{
    CalculatedTax, PriceInput, calculate, math_round, proportional_tax_rules,
};
use veyndo_pricing::quote::{LinePriceUpdate, QuoteTotals};
use veyndo_pricing::service::conversion as currencies;
/// The quote facts an app adjustment reads, each as leniently as the adjustment always read it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AdjustedQuote {
    #[serde(deserialize_with = "lenient")]
    price: QuotePrice,
    #[serde(deserialize_with = "lenient")]
    app_shipping_adjustment_minor: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    shipping_costs: ShippingCosts,
    #[serde(deserialize_with = "lenient")]
    discount_total: Option<f64>,
    line_items: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuotePrice {
    #[serde(deserialize_with = "lenient")]
    currency_scale: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    tax_status: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ShippingCosts {
    #[serde(deserialize_with = "lenient")]
    total_price: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AdjustedLine {
    #[serde(deserialize_with = "lenient")]
    quantity: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    price: AdjustedLinePrice,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AdjustedLinePrice {
    #[serde(deserialize_with = "lenient")]
    total_price: Option<f64>,
    calculated_taxes: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LenientTax {
    #[serde(deserialize_with = "lenient")]
    tax_rate: Option<f64>,
    #[serde(deserialize_with = "lenient")]
    price: Option<f64>,
    #[serde(deserialize_with = "lenient")]
    tax: Option<f64>,
}

/// The adjusted lines summed into the quote totals; these values are required.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummedQuote {
    line_items: Vec<SummedLine>,
}

#[derive(Deserialize)]
struct SummedLine {
    price: SummedPrice,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummedPrice {
    total_price: f64,
    calculated_taxes: Vec<SummedTax>,
}

#[derive(Deserialize)]
struct SummedTax {
    tax: f64,
}

/// The app adjustments recorded on the quote itself.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Recorded {
    #[serde(skip_serializing_if = "Option::is_none")]
    app_shipping_adjustment_minor: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount_total: Option<f64>,
}

/// The new line prices of a price or discount adjustment, in line order.
fn adjusted_lines(
    quote: &AdjustedQuote,
    kind: &str,
    minor: i64,
    scale: i32,
) -> Result<Vec<LinePriceUpdate>> {
    let factor = 10_f64.powi(scale);
    if minor < 0 {
        return Err(bad(
            "Price/discount hooks return a nonnegative adjustment; the hook determines its direction",
        ));
    }
    let lines = quote
        .line_items
        .as_array()
        .ok_or(bad("Quote items missing"))?
        .iter()
        .map(view::<AdjustedLine>)
        .collect::<Vec<_>>();
    let currency = quote.price.currency.as_deref().unwrap_or("");
    let totals: Vec<i64> = lines
        .iter()
        .map(|l| {
            currencies::amount(l.price.total_price.unwrap_or(0.), currency, scale as u8)
                .map(|m| m.minor())
        })
        .collect::<Result<_>>()?;
    let sum = totals
        .iter()
        .try_fold(0i64, |sum, n| sum.checked_add(*n))
        .ok_or(bad("Hook total overflow"))?;
    if minor > sum {
        return Err(bad("App adjustment exceeds current goods value"));
    }
    // The existing allocator distributes exact minor units without creating a second rounding implementation.
    let after = veyndo_pricing::discount::allocate(&totals, minor);
    if kind == "price" {
        sum.checked_add(minor)
            .ok_or(bad("App adjusted total overflow"))?;
    }
    let targets = totals
        .iter()
        .zip(after)
        .map(|(before, after)| {
            if kind == "price" {
                before
                    .checked_add(before - after)
                    .ok_or(bad("App adjusted line overflow"))
            } else {
                Ok(after)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let b2b = quote.price.tax_status.as_deref() == Some("net");
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let taxes: Vec<CalculatedTax> = line
                .price
                .calculated_taxes
                .as_array()
                .ok_or(bad("Quote taxes missing"))?
                .iter()
                .map(view::<LenientTax>)
                .map(|r| CalculatedTax {
                    tax_rate: r.tax_rate.unwrap_or(0.),
                    price: r.price.unwrap_or(0.),
                    tax: r.tax.unwrap_or(0.),
                })
                .collect();
            let tax_rules = proportional_tax_rules(&taxes, totals[i] as f64 / factor);
            let calc = calculate(&PriceInput {
                price: targets[i] as f64 / factor,
                quantity: 1,
                decimals: scale,
                interval: 1. / factor,
                tax_rules: Some(tax_rules),
                gross: !b2b,
                ..PriceInput::default()
            });
            let quantity = line.quantity.unwrap_or(1).max(1) as f64;
            Ok(LinePriceUpdate::new(
                calc.total_price,
                quantity,
                scale,
                &calc.calculated_taxes,
            ))
        })
        .collect()
}

/// Applies an admitted integer app adjustment to a priced quote through native pricing and tax calculation.
pub fn apply(q: &mut Value, kind: &str, minor: i64) -> Result<()> {
    let quote: AdjustedQuote = view(q);
    let scale = quote
        .price
        .currency_scale
        .ok_or(bad("Quote scale missing"))? as i32;
    let factor = 10_f64.powi(scale);
    if kind == "validation" {
        if minor != 0 {
            return Err(bad("Validation hook cannot change money"));
        }
        return Ok(());
    }
    let mut recorded = Recorded {
        app_shipping_adjustment_minor: None,
        discount_total: None,
    };
    if kind == "shipping" {
        let combined = quote
            .app_shipping_adjustment_minor
            .unwrap_or(0)
            .checked_add(minor)
            .filter(|n| n.unsigned_abs() <= 100_000_000)
            .ok_or(bad("Shipping hook adjustment overflow"))?;
        if quote.shipping_costs.total_price.unwrap_or(0.) + minor as f64 / factor < 0. {
            return Err(bad("Shipping hook cannot make delivery cost negative"));
        }
        recorded.app_shipping_adjustment_minor = Some(combined);
    } else {
        let updates = adjusted_lines(&quote, kind, minor, scale)?;
        if let Some(items) = q.get_mut("lineItems").and_then(Value::as_array_mut) {
            for (item, update) in items.iter_mut().zip(updates) {
                let price = item
                    .as_object_mut()
                    .map(|line| line.entry("price").or_insert(Value::Null))
                    .ok_or(bad("Quote taxes missing"))?;
                merge(price, update)?;
            }
        }
        if kind == "discount" {
            recorded.discount_total = Some(math_round(
                quote.discount_total.unwrap_or(0.) + minor as f64 / factor,
                scale,
            ));
        }
    }
    // Strip the previous delivery components; the shared delivery owner recalculates them exactly once next.
    let summed: SummedQuote = serde_json::from_value(q.clone()).map_err(|_| {
        Error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Priced quote is incomplete".into(),
        )
    })?;
    let positions = math_round(
        summed.line_items.iter().map(|l| l.price.total_price).sum(),
        scale,
    );
    let tax = math_round(
        summed
            .line_items
            .iter()
            .flat_map(|l| &l.price.calculated_taxes)
            .map(|t| t.tax)
            .sum(),
        scale,
    );
    let net = quote.price.tax_status.as_deref() == Some("net");
    merge(q, recorded)?;
    let price = q
        .as_object_mut()
        .map(|o| o.entry("price").or_insert(Value::Null))
        .ok_or(bad("Quote scale missing"))?;
    merge(price, QuoteTotals::new(positions, tax, scale, net))
}
#[cfg(test)]
mod tests {
    use super::apply;
    use serde_json::{Value, json};
    use veyndo_core::{Result, StatusCode, bad};
    use veyndo_pricing::pricing::{PriceInput, calculate, math_round, proportional_tax_rules};
    use veyndo_pricing::service::conversion as currencies;

    // The previous untyped implementation, kept as the oracle of the typed one.
    fn legacy(q: &mut Value, kind: &str, minor: i64) -> Result<()> {
        let scale = q["price"]["currencyScale"]
            .as_u64()
            .ok_or(bad("Quote scale missing"))? as i32;
        let factor = 10_f64.powi(scale);
        if kind == "validation" {
            if minor != 0 {
                return Err(bad("Validation hook cannot change money"));
            }
            return Ok(());
        }
        if kind == "shipping" {
            let previous = q["appShippingAdjustmentMinor"].as_i64().unwrap_or(0);
            let combined = previous
                .checked_add(minor)
                .filter(|n| n.unsigned_abs() <= 100_000_000)
                .ok_or(bad("Shipping hook adjustment overflow"))?;
            if q["shippingCosts"]["totalPrice"].as_f64().unwrap_or(0.) + minor as f64 / factor < 0.
            {
                return Err(bad("Shipping hook cannot make delivery cost negative"));
            }
            q["appShippingAdjustmentMinor"] = json!(combined);
        } else {
            if minor < 0 {
                return Err(bad(
                    "Price/discount hooks return a nonnegative adjustment; the hook determines its direction",
                ));
            }
            let lines = q["lineItems"]
                .as_array()
                .ok_or(bad("Quote items missing"))?;
            let totals: Vec<i64> = lines
                .iter()
                .map(|l| {
                    currencies::amount(
                        l["price"]["totalPrice"].as_f64().unwrap_or(0.),
                        q["price"]["currency"].as_str().unwrap_or(""),
                        scale as u8,
                    )
                    .map(|m| m.minor())
                })
                .collect::<Result<_>>()?;
            let sum = totals
                .iter()
                .try_fold(0i64, |sum, n| sum.checked_add(*n))
                .ok_or(bad("Hook total overflow"))?;
            if minor > sum {
                return Err(bad("App adjustment exceeds current goods value"));
            }
            // The existing allocator distributes exact minor units without creating a second rounding implementation.
            let after = veyndo_pricing::discount::allocate(&totals, minor);
            if kind == "price" {
                sum.checked_add(minor)
                    .ok_or(bad("App adjusted total overflow"))?;
            }
            let targets = totals
                .iter()
                .zip(after)
                .map(|(before, after)| {
                    if kind == "price" {
                        before
                            .checked_add(before - after)
                            .ok_or(bad("App adjusted line overflow"))
                    } else {
                        Ok(after)
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            let b2b = q["price"]["taxStatus"] == "net";
            for (i, line) in q["lineItems"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .enumerate()
            {
                let target = targets[i];
                let rules = line["price"]["calculatedTaxes"]
                    .as_array()
                    .ok_or(bad("Quote taxes missing"))?;
                let taxes: Vec<veyndo_pricing::pricing::CalculatedTax> = rules
                    .iter()
                    .map(|r| veyndo_pricing::pricing::CalculatedTax {
                        tax_rate: r["taxRate"].as_f64().unwrap_or(0.),
                        price: r["price"].as_f64().unwrap_or(0.),
                        tax: r["tax"].as_f64().unwrap_or(0.),
                    })
                    .collect();
                let tax_rules = proportional_tax_rules(&taxes, totals[i] as f64 / factor);
                let calc = calculate(&PriceInput {
                    price: target as f64 / factor,
                    quantity: 1,
                    decimals: scale,
                    interval: 1. / factor,
                    tax_rules: Some(tax_rules),
                    gross: !b2b,
                    ..PriceInput::default()
                });
                let qty = line["quantity"].as_u64().unwrap_or(1).max(1) as f64;
                line["price"]["totalPrice"] = json!(calc.total_price);
                line["price"]["unitPrice"] = json!(math_round(calc.total_price / qty, scale));
                line["price"]["calculatedTaxes"] = json!(
                    calc.calculated_taxes
                        .iter()
                        .map(|t| json!({"tax":t.tax,"taxRate":t.tax_rate,"price":t.price}))
                        .collect::<Vec<_>>()
                );
            }
            if kind == "discount" {
                q["discountTotal"] = json!(math_round(
                    q["discountTotal"].as_f64().unwrap_or(0.) + minor as f64 / factor,
                    scale
                ));
            }
        }
        // Strip the previous delivery components; the shared delivery owner recalculates them exactly once next.
        let lines = q["lineItems"].as_array().unwrap();
        let positions = math_round(
            lines
                .iter()
                .map(|l| l["price"]["totalPrice"].as_f64().unwrap())
                .sum(),
            scale,
        );
        let tax = math_round(
            lines
                .iter()
                .flat_map(|l| l["price"]["calculatedTaxes"].as_array().unwrap())
                .map(|t| t["tax"].as_f64().unwrap())
                .sum(),
            scale,
        );
        let b2b = q["price"]["taxStatus"] == "net";
        q["price"]["positionPrice"] = json!(positions);
        q["price"]["tax"] = json!(tax);
        q["price"]["netPrice"] = json!(if b2b {
            positions
        } else {
            math_round(positions - tax, scale)
        });
        q["price"]["totalPrice"] = json!(if b2b {
            math_round(positions + tax, scale)
        } else {
            positions
        });
        Ok(())
    }

    #[test]
    fn typed_adjustments_equal_the_untyped_ones() {
        let mixed = json!({"totalPrice":119.,"unitPrice":119.,"listPrice":{"price":130},"calculatedTaxes":[{"taxRate":19.,"tax":9.5,"price":59.5},{"taxRate":7.,"tax":3.89,"price":59.5}]});
        let mut quotes = vec![quote()];
        let mut q = quote();
        q["lineItems"][0]["price"] = mixed;
        q["lineItems"].as_array_mut().unwrap().push(json!({"quantity":3,"label":"kept","price":{"totalPrice":30.,"calculatedTaxes":[{"taxRate":7.,"tax":1.96,"price":30.}]}}));
        q["discountTotal"] = json!(2.5);
        q["appShippingAdjustmentMinor"] = json!(-100);
        quotes.push(q.clone());
        q["price"]["taxStatus"] = json!("net");
        q["price"]["currencyScale"] = json!(0);
        q["price"]["currency"] = json!("JPY");
        quotes.push(q);
        let mut odd = quote();
        odd["lineItems"].as_array_mut().unwrap().push(json!({"quantity":"2","price":{"totalPrice":"5","calculatedTaxes":["x",{"taxRate":"19"}]}}));
        odd["shippingCosts"] = json!([4.9]);
        odd["discountTotal"] = json!("1");
        quotes.push(odd);
        let mut no_taxes = quote();
        no_taxes["lineItems"]
            .as_array_mut()
            .unwrap()
            .push(json!({"price":{"totalPrice":5.}}));
        quotes.push(no_taxes);
        quotes.push(json!({"price":{"currencyScale":2},"lineItems":"none"}));
        quotes.push(json!({"price":{"currencyScale":2.0}}));
        for q in &quotes {
            for kind in ["validation", "shipping", "price", "discount", "other"] {
                for minor in [0, 1, 99, 1190, -491, -1, 20_000, 200_000_000] {
                    let (mut typed, mut old) = (q.clone(), q.clone());
                    let a = apply(&mut typed, kind, minor);
                    let Ok(b) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        legacy(&mut old, kind, minor)
                    })) else {
                        // The untyped reader panicked here; the typed one refuses with an error instead.
                        assert_eq!(
                            a.unwrap_err().0,
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "{kind} {minor} {q}"
                        );
                        continue;
                    };
                    assert_eq!(
                        a.as_ref().err().map(|e| &e.1),
                        b.as_ref().err().map(|e| &e.1),
                        "{kind} {minor} {q}"
                    );
                    if a.is_ok() {
                        assert_eq!(typed, old, "{kind} {minor} {q}");
                    }
                }
            }
        }
    }
    fn quote() -> Value {
        json!({"price":{"currency":"EUR","currencyScale":2,"taxStatus":"gross","positionPrice":119.,"totalPrice":123.9,"tax":19.78,"netPrice":104.12},"lineItems":[{"quantity":1,"price":{"totalPrice":119.,"unitPrice":119.,"calculatedTaxes":[{"taxRate":19.,"tax":19.,"price":119.}]}}],"shippingCosts":{"totalPrice":4.9}})
    }
    #[test]
    fn mixed_tax_allocation_is_preserved_by_native_recalculation() {
        let mut q = quote();
        q["lineItems"][0]["price"] = json!({"totalPrice":119.,"unitPrice":119.,"calculatedTaxes":[{"taxRate":19.,"tax":9.5,"price":59.5},{"taxRate":7.,"tax":3.89,"price":59.5}]});
        apply(&mut q, "discount", 1190).unwrap();
        let taxes = q["lineItems"][0]["price"]["calculatedTaxes"]
            .as_array()
            .unwrap();
        assert_eq!(taxes.len(), 2);
        assert_eq!(taxes[0]["price"], 53.55);
        assert_eq!(taxes[1]["price"], 53.55);
        assert_eq!(taxes[0]["tax"], 8.55);
        assert_eq!(taxes[1]["tax"], 3.5);
    }
    #[test]
    fn native_app_adjustments_conserve_units_tax_and_goods_bounds() {
        let mut q = quote();
        apply(&mut q, "discount", 1190).unwrap();
        assert_eq!(q["price"]["totalPrice"], 107.1);
        assert_eq!(q["price"]["tax"], 17.1);
        assert_eq!(q["price"]["netPrice"], 90.);
        apply(&mut q, "price", 1071).unwrap();
        assert_eq!(q["price"]["totalPrice"], 117.81);
        assert!(apply(&mut q, "price", 20000).is_err());
        assert!(apply(&mut q, "discount", -1).is_err());
        assert!(apply(&mut q, "shipping", -491).is_err());
        assert!(apply(&mut q, "validation", 1).is_err());
    }
    #[test]
    fn overflowing_app_price_is_rejected_before_mutating_quote() {
        let mut q = quote();
        q["lineItems"][0]["price"]["totalPrice"] = json!(80_000_000_000_000_000.0);
        let original = q.clone();
        let error = apply(&mut q, "price", 2_000_000_000_000_000_000).unwrap_err();
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert_eq!(error.1, "App adjusted total overflow");
        assert_eq!(q, original);
    }
}
