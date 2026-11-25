//! Server-authoritative coupons and automatic campaigns, with deterministic discounts and atomic usage limits.
use crate::store::{facts, rule_snapshot};
use crate::types::promotion::Promotion;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Error, Result, StatusCode, bad};
use veyndo_pricing::pricing::{PriceInput, calculate, math_round};
use veyndo_pricing::quote::{LinePriceUpdate, QuoteTotals};
use veyndo_shop::types::cart::StoredCart;

/// The promotions that apply to this cart right now, highest priority first.
async fn applicable(
    tx: &mut sqlx::PgConnection,
    c: &StoredCart,
    q: &Value,
) -> Result<Vec<(String, Promotion)>> {
    let rows = sqlx::query!(
        "SELECT id,data FROM commerce_promotions WHERE tenant=$1 ORDER BY id FOR UPDATE",
        &c.tenant
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut evaluated = q.clone();
    if !rows.is_empty() {
        let mut rule_facts = facts::rule_facts(tx, c, q).await?;
        rule_snapshot::attach(
            tx,
            &c.tenant,
            &json!(rows.iter().map(|r| r.data.clone()).collect::<Vec<_>>()),
            &mut rule_facts,
        )
        .await?;
        merge(&mut evaluated, json!({ "ruleFacts": rule_facts }))?;
    }
    let quote_currency = view::<QuotePriceOnly>(q).price.currency;
    let mut applicable = vec![];
    for r in rows {
        let p: Promotion =
            serde_json::from_value(r.data.clone()).map_err(|_| bad("Invalid promotion"))?;
        // New statement snapshot after row locks: concurrent checkout sees the preceding committed use.
        let uses: i64 = sqlx::query_scalar!(
            r#"SELECT count(*) AS "uses!" FROM promotion_uses WHERE tenant=$1 AND promotion=$2"#,
            &c.tenant,
            r.id.clone()
        )
        .fetch_one(&mut *tx)
        .await?;
        if !p.active
            || p.source_import.as_ref().is_some_and(|s| {
                !s.activation_reviewed || quote_currency.as_deref() != Some(s.currency.as_str())
            })
            || p.max_uses.is_some_and(|n| uses >= n)
            || p.code.as_ref().is_some_and(|v| !c.data.coupons.contains(v))
            || !p.rule.checked_matches(c, &evaluated)?
        {
            continue;
        }
        let valid:bool=sqlx::query_scalar!(r#"SELECT ($1::text IS NULL OR now()>=$1::timestamptz) AND ($2::text IS NULL OR now()<=$2::timestamptz) AS "valid!""#, p.start.as_deref(), p.end.as_deref()).fetch_one(&mut *tx).await?;
        if valid {
            applicable.push((r.id.clone(), p));
        }
    }
    applicable.sort_by(|a, b| b.1.priority.cmp(&a.1.priority).then(a.0.cmp(&b.0)));
    Ok(applicable)
}
/// The parts of a priced quote that promotions read. Line totals and taxes are required:
/// a quote without them is refused, never discounted from defaults.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromotedQuote {
    #[serde(default, deserialize_with = "lenient")]
    price: QuoteSettings,
    #[serde(default, deserialize_with = "lenient")]
    currency_context: CurrencyContext,
    line_items: Vec<PricedLine>,
}

/// The quote price settings alone, for promotion admission.
#[derive(Deserialize, Default)]
#[serde(default)]
struct QuotePriceOnly {
    #[serde(deserialize_with = "lenient")]
    price: QuoteSettings,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuoteSettings {
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    tax_status: Option<String>,
    #[serde(deserialize_with = "lenient")]
    currency_scale: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CurrencyContext {
    #[serde(deserialize_with = "lenient")]
    pricing_factor: Option<String>,
    #[serde(deserialize_with = "lenient")]
    factor: Option<String>,
}

#[derive(Deserialize)]
struct PricedLine {
    #[serde(default, deserialize_with = "lenient")]
    quantity: Option<u64>,
    price: LinePrice,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinePrice {
    total_price: f64,
    calculated_taxes: Vec<LineTax>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LineTax {
    tax: f64,
    #[serde(default, deserialize_with = "lenient")]
    tax_rate: Option<f64>,
}

/// A line while discounts are spread: its current total, first tax rate and the price it now carries.
struct Line {
    total: f64,
    tax_rate: f64,
    quantity: f64,
    update: Option<LinePriceUpdate>,
}

/// What the promotions decided, merged into the quote.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    promotion_free_shipping: Option<bool>,
    discounts: Vec<Value>,
    discount_total: f64,
    coupon_codes: &'a [String],
}

fn incomplete() -> Error {
    Error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Priced quote is incomplete".into(),
    )
}

/// Spreads a discount amount over the lines, recalculates their taxes and returns the new position total and tax.
fn discount_lines(
    lines: &mut [Line],
    amount: f64,
    (decimals, factor, b2b): (i32, f64, bool),
) -> (f64, f64) {
    let totals = lines
        .iter()
        .map(|l| (l.total * factor).round() as i64)
        .collect::<Vec<_>>();
    let allocated = veyndo_pricing::discount::allocate(&totals, (amount * factor).round() as i64);
    let mut after = 0.;
    let mut tax = 0.;
    for (index, line) in lines.iter_mut().enumerate() {
        let calc = calculate(&PriceInput {
            price: allocated[index] as f64 / factor,
            decimals,
            interval: 1. / factor,
            quantity: 1,
            tax_rate: line.tax_rate,
            gross: !b2b,
            ..PriceInput::default()
        });
        line.total = calc.total_price;
        line.tax_rate = calc.calculated_taxes.first().map_or(0., |t| t.tax_rate);
        line.update = Some(LinePriceUpdate::new(
            calc.total_price,
            line.quantity,
            decimals,
            &calc.calculated_taxes,
        ));
        after += calc.total_price;
        tax += calc.tax;
    }
    (math_round(after, decimals), math_round(tax, decimals))
}

/// Applies the promotions in order to a priced quote: discounted lines, totals, discounts and coupon codes.
fn apply(q: &mut Value, applicable: Vec<(String, Promotion)>, coupons: &[String]) -> Result<()> {
    let quote: PromotedQuote = serde_json::from_value(q.clone()).map_err(|_| incomplete())?;
    let b2b = quote.price.tax_status.as_deref() == Some("net");
    let decimals = quote.price.currency_scale.unwrap_or(2) as i32;
    let factor = 10_f64.powi(decimals);
    let units = |rate: &Option<String>| -> Result<f64> {
        Ok(rate
            .as_deref()
            .map(veyndo_pricing::types::currency::rate_units)
            .transpose()?
            .unwrap_or(100_000_000) as f64)
    };
    let pricing_rate = units(&quote.currency_context.pricing_factor)?;
    let rate = units(&quote.currency_context.factor)? / pricing_rate;
    let positions = quote
        .line_items
        .iter()
        .map(|l| l.price.total_price)
        .sum::<f64>();
    let tax = quote
        .line_items
        .iter()
        .flat_map(|l| &l.price.calculated_taxes)
        .map(|t| t.tax)
        .sum::<f64>();
    let original = math_round(if b2b { positions + tax } else { positions }, decimals);
    let mut current = QuoteTotals {
        position_price: math_round(positions, decimals),
        total_price: original,
        tax: math_round(tax, decimals),
        net_price: math_round(if b2b { positions } else { positions - tax }, decimals),
    };
    let mut lines = quote
        .line_items
        .iter()
        .map(|l| Line {
            total: l.price.total_price,
            tax_rate: l
                .price
                .calculated_taxes
                .first()
                .and_then(|t| t.tax_rate)
                .unwrap_or(0.),
            quantity: l.quantity.unwrap_or(1) as f64,
            update: None,
        })
        .collect::<Vec<_>>();
    let mut free_shipping = None;
    let mut discounts = vec![];
    for (id, p) in applicable {
        if p.kind == "free_shipping" {
            free_shipping = Some(true);
            discounts.push(json!({"id":id,"name":p.name,"kind":p.kind,"amount":0}));
        } else {
            let total = current.position_price;
            if total <= 0. {
                continue;
            }
            let ratio = if p.kind == "percentage" {
                p.amount / 100.
            } else {
                (p.amount * rate / total).min(1.)
            };
            let (after, tax) = discount_lines(&mut lines, total * ratio, (decimals, factor, b2b));
            current = QuoteTotals::new(after, tax, decimals, b2b);
            discounts.push(
                json!({"id":id,"name":p.name,"kind":p.kind,"amount":math_round(total-after,decimals)}),
            );
        }
        if p.exclusive {
            break;
        }
    }
    let discount_total = math_round(original - current.total_price, decimals);
    let stored = q.as_object_mut().ok_or_else(incomplete)?;
    if let Some(items) = stored.get_mut("lineItems").and_then(Value::as_array_mut) {
        for (item, line) in items.iter_mut().zip(lines) {
            if let (Some(update), Some(price)) = (line.update, item.get_mut("price")) {
                merge(price, update)?;
            }
        }
    }
    merge(stored.entry("price").or_insert(Value::Null), current)?;
    merge(
        q,
        Outcome {
            promotion_free_shipping: free_shipping,
            discounts,
            discount_total,
            coupon_codes: coupons,
        },
    )
}

pub async fn promote(tx: &mut sqlx::PgConnection, c: &StoredCart, mut q: Value) -> Result<Value> {
    let applicable = applicable(tx, c, &q).await?;
    apply(&mut q, applicable, &c.data.coupons)?;
    Ok(q)
}
/// The JSON fields `record_uses` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RecordUsesInput {
    discounts: Value,
}

pub async fn record_uses(
    tx: &mut sqlx::PgConnection,
    t: &str,
    order: &str,
    q: &Value,
) -> Result<()> {
    let input: RecordUsesInput = view(q);
    for d in input.discounts.as_array().unwrap_or(&vec![]) {
        let used: Used = view(d);
        sqlx::query!("INSERT INTO promotion_uses(tenant,promotion,order_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", t, used.id.as_str(), order).execute(&mut *tx).await?;
    }
    Ok(())
}

/// The promotion a quote discount came from.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Used {
    id: Value,
}

#[cfg(test)]
#[path = "promotion_property_tests.rs"]
mod properties;
#[cfg(test)]
mod tests {
    use super::apply;
    use crate::types::promotion::Promotion;
    use serde_json::{Value, json};
    use veyndo_core::Result;
    use veyndo_pricing::pricing::{PriceInput, calculate, math_round};

    // The previous untyped implementation, kept as the oracle of the typed one.
    /// Spreads a discount amount over the lines, recalculates their taxes and the totals, and returns the new position total.
    fn discount_lines(
        q: &mut Value,
        amount: f64,
        (decimals, factor, b2b): (i32, f64, bool),
    ) -> f64 {
        let totals = q["lineItems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| (l["price"]["totalPrice"].as_f64().unwrap() * factor).round() as i64)
            .collect::<Vec<_>>();
        let allocated =
            veyndo_pricing::discount::allocate(&totals, (amount * factor).round() as i64);
        let mut after = 0.;
        let mut tax = 0.;
        for (index, line) in q["lineItems"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            let tax_rate = line["price"]["calculatedTaxes"][0]["taxRate"]
                .as_f64()
                .unwrap_or(0.);
            let discounted = allocated[index] as f64 / factor;
            let calc = calculate(&PriceInput {
                price: discounted,
                decimals,
                interval: 1. / factor,
                quantity: 1,
                tax_rate,
                gross: !b2b,
                ..PriceInput::default()
            });
            let quantity = line["quantity"].as_u64().unwrap_or(1) as f64;
            line["price"]["totalPrice"] = json!(calc.total_price);
            line["price"]["unitPrice"] = json!(math_round(calc.total_price / quantity, decimals));
            line["price"]["calculatedTaxes"] = json!(
                calc.calculated_taxes
                    .iter()
                    .map(|t| json!({"tax":t.tax,"taxRate":t.tax_rate,"price":t.price}))
                    .collect::<Vec<_>>()
            );
            after += calc.total_price;
            tax += calc.tax;
        }
        after = math_round(after, decimals);
        tax = math_round(tax, decimals);
        q["price"]["positionPrice"] = json!(after);
        q["price"]["tax"] = json!(tax);
        q["price"]["netPrice"] = json!(if b2b {
            after
        } else {
            math_round(after - tax, decimals)
        });
        q["price"]["totalPrice"] = json!(if b2b {
            math_round(after + tax, decimals)
        } else {
            after
        });
        after
    }
    fn legacy(
        mut q: Value,
        applicable: Vec<(String, Promotion)>,
        coupons: &[String],
    ) -> Result<Value> {
        let b2b = q["price"]["taxStatus"] == "net";
        let decimals = q["price"]["currencyScale"].as_i64().unwrap_or(2) as i32;
        let factor = 10_f64.powi(decimals);
        let pricing_rate = q["currencyContext"]["pricingFactor"]
            .as_str()
            .map(veyndo_pricing::types::currency::rate_units)
            .transpose()?
            .unwrap_or(100_000_000) as f64;
        let rate = q["currencyContext"]["factor"]
            .as_str()
            .map(veyndo_pricing::types::currency::rate_units)
            .transpose()?
            .unwrap_or(100_000_000) as f64
            / pricing_rate;
        let positions = q["lineItems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["price"]["totalPrice"].as_f64().unwrap())
            .sum::<f64>();
        let tax = q["lineItems"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|l| l["price"]["calculatedTaxes"].as_array().unwrap())
            .map(|t| t["tax"].as_f64().unwrap())
            .sum::<f64>();
        let original = math_round(if b2b { positions + tax } else { positions }, decimals);
        q["price"]["positionPrice"] = json!(math_round(positions, decimals));
        q["price"]["totalPrice"] = json!(original);
        q["price"]["tax"] = json!(math_round(tax, decimals));
        q["price"]["netPrice"] = json!(math_round(
            if b2b { positions } else { positions - tax },
            decimals
        ));
        let mut discounts = vec![];
        for (id, p) in applicable {
            if p.kind == "free_shipping" {
                q["promotionFreeShipping"] = json!(true);
                discounts.push(json!({"id":id,"name":p.name,"kind":p.kind,"amount":0}));
            } else {
                let total = q["price"]["positionPrice"].as_f64().unwrap();
                if total <= 0. {
                    continue;
                }
                let ratio = if p.kind == "percentage" {
                    p.amount / 100.
                } else {
                    (p.amount * rate / total).min(1.)
                };
                let after = discount_lines(&mut q, total * ratio, (decimals, factor, b2b));
                discounts.push(
                    json!({"id":id,"name":p.name,"kind":p.kind,"amount":math_round(total-after,decimals)}),
                );
            }
            if p.exclusive {
                break;
            }
        }
        q["discounts"] = json!(discounts);
        q["discountTotal"] = json!(math_round(
            original - q["price"]["totalPrice"].as_f64().unwrap(),
            decimals
        ));
        q["couponCodes"] = json!(coupons);
        Ok(q)
    }

    fn promotion(kind: &str, amount: f64, exclusive: bool) -> (String, Promotion) {
        let p = serde_json::from_value(json!({"name":{"en":kind},"active":true,"code":null,"kind":kind,"amount":amount,
            "rule":{"type":"alwaysValid"},"exclusive":exclusive,"priority":1,"maxUses":null,"start":null,"end":null}))
        .unwrap();
        (format!("{kind}-{amount}"), p)
    }

    fn quote(net: bool, scale: i64, context: Value) -> Value {
        let line = |total: f64, rate: f64, quantity: Value| {
            let tax = math_round(total - total / (1. + rate / 100.), scale as i32);
            json!({"id":"l","quantity":quantity,"label":"kept","price":{"unitPrice":total,"totalPrice":total,"listPrice":null,
                "calculatedTaxes":[{"tax":tax,"taxRate":rate,"price":total}]}})
        };
        json!({"price":{"taxStatus":if net {"net"} else {"gross"},"currencyScale":scale,"currency":"EUR"},"currencyContext":context,
            "lineItems":[line(19.99, 19., json!(2)), line(5.0, 7., json!(1)), line(0.01, 19., json!("x")),
                {"quantity":3,"price":{"totalPrice":12.0,"calculatedTaxes":[]}}],"keep":true})
    }

    #[test]
    fn typed_promotions_compute_exactly_what_the_untyped_ones_did() {
        let sets = [
            vec![],
            vec![promotion("percentage", 10., false)],
            vec![
                promotion("percentage", 10., false),
                promotion("absolute", 3.5, false),
                promotion("free_shipping", 0., false),
            ],
            vec![
                promotion("absolute", 500., true),
                promotion("percentage", 50., false),
            ],
            vec![
                promotion("free_shipping", 0., true),
                promotion("percentage", 20., false),
            ],
        ];
        let contexts = [
            json!(null),
            json!({"factor":"1.10","pricingFactor":"1.00"}),
            json!({"factor":7,"pricingFactor":["x"]}),
        ];
        let coupons = vec!["SAVE".to_string()];
        for net in [false, true] {
            for scale in [0, 2, 3] {
                for context in &contexts {
                    for set in &sets {
                        let q = quote(net, scale, context.clone());
                        let mut typed = q.clone();
                        apply(&mut typed, set.clone(), &coupons).unwrap();
                        assert_eq!(
                            typed,
                            legacy(q, set.clone(), &coupons).unwrap(),
                            "{net} {scale} {context}"
                        );
                    }
                }
            }
        }
        let mut broken =
            json!({"price":{},"lineItems":[{"price":{"totalPrice":"1","calculatedTaxes":[]}}]});
        assert!(apply(&mut broken, vec![], &coupons).is_err());
    }
}
