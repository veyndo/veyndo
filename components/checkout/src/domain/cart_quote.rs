//! The authoritative cart quote: quantity pricing, app configuration fees and localized totals. Totals are added as whole minor units.
use crate::domain::quote_view::LineView;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{merge, view};
use veyndo_core::{Result, bad};
use veyndo_pricing::catalog::Product;
use veyndo_pricing::decimal::{Decimal, Ratio};
use veyndo_pricing::domain::context;
use veyndo_pricing::money::Minor;
use veyndo_pricing::pricing::{PriceInput, calculate};
use veyndo_pricing::service::conversion::{
    ancillary, context as currency_context, convert, convert_from, product_price,
};
use veyndo_pricing::types::currency::{Config, Definition, scale};
use veyndo_pricing::verified_kernel;
use veyndo_shop::Settings;
use veyndo_shop::types::cart::{AppConfiguration, StoredCart};

/// The app an app configuration belongs to.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ConfigApp {
    app: Value,
}

/// The app configurations of a quote line: the first by app id, and all of them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LineConfigurations<'a> {
    configuration: &'a Value,
    app_configurations: &'a [Value],
}

/// The priced cart: line prices with tiers, app configuration fees and taxes, totals summed in whole minor units.
pub fn quote(c: &StoredCart, ps: &[Product], settings: &Settings) -> Result<Value> {
    let b2b = settings.is_business(&c.data.group);
    let cfg = &settings.currencies;
    let currency = cfg.selected(&c.data.currency)?;
    let decimals = currency.scale;
    let interval = Decimal::new(1, decimals.into()).map_err(bad)?;
    let mut lines = vec![];
    // Totals are added as whole minor units by the proved sum rule; the calculators deliver rounded amounts.
    let mut total = 0_u64;
    let mut taxes = 0_u64;
    let one = Ratio::integer(1);
    let hundred = Ratio::integer(100);
    for i in &c.data.items {
        let p = ps
            .iter()
            .find(|p| p.id == i.id)
            .ok_or(bad(format!("Unknown product {}", i.id)))?;
        if normalized_quantity(p, i.quantity)? != i.quantity {
            return Err(bad(
                "Quantity does not match product minimum/steps; update cart",
            ));
        }
        let mut rule_ids = vec![c.data.group.clone()];
        rule_ids.extend(if b2b {
            vec!["business".into()]
        } else {
            vec!["consumer".into()]
        });
        let tier = context::select_tier(&p.advanced_prices, &rule_ids, i.quantity);
        let tier_discount = tier
            .map(|t| Decimal::from_wire(t.discount))
            .transpose()
            .map_err(bad)?
            .unwrap_or(Decimal::ZERO);
        let discount = one.checked_sub(tier_discount.ratio()).map_err(bad)?;
        let tax_rate = p.tax_rate;
        // A B2B line is priced from the gross amount without its tax: `gross / (1 + rate / 100)`, exact.
        let net_of = |gross: Ratio| -> Result<Ratio> {
            if b2b {
                gross
                    .checked_div(
                        one.checked_add(tax_rate.ratio().checked_div(hundred).map_err(bad)?)
                            .map_err(bad)?,
                    )
                    .map_err(bad)
            } else {
                Ok(gross)
            }
        };
        let gross = product_price(p, cfg, currency)?.ratio();
        let ancillary_price = |field: &str, value: Option<Decimal>| -> Result<Option<Ratio>> {
            ancillary(p, field, value, cfg, currency)?
                .map(|v| net_of(v.ratio()))
                .transpose()
        };
        let mut calc = calculate(&PriceInput {
            price: net_of(gross)?.checked_mul(discount).map_err(bad)?,
            quantity: i.quantity,
            tax_rate,
            gross: !b2b,
            calculated: true,
            decimals: decimals.into(),
            interval,
            round_for_net: false,
            list_price: ancillary_price("listPrice", p.list_price)?,
            regulation_price: ancillary_price("regulationPrice", p.regulation_price)?,
            reference: p.reference_price.clone(),
            ..PriceInput::default()
        })
        .map_err(bad)?;
        for config in c
            .data
            .app_configurations
            .values()
            .filter(|v| v.product_id == i.id)
        {
            let fee_gross = configuration_price(config, cfg, currency)?.ratio();
            let fee = calculate(&PriceInput {
                price: net_of(fee_gross)?,
                quantity: i.quantity,
                decimals: decimals.into(),
                interval,
                tax_rate,
                gross: !b2b,
                ..PriceInput::default()
            })
            .map_err(bad)?;
            // The fee is added in whole minor units (the previous decimal sum rounded once).
            calc.unit_price = calc.unit_price.checked_add(fee.unit_price).map_err(bad)?;
            calc.total_price = calc.total_price.checked_add(fee.total_price).map_err(bad)?;
            calc.tax = calc.tax.checked_add(fee.tax).map_err(bad)?;
            calc.calculated_taxes.extend(fee.calculated_taxes);
        }
        let minor = |amount: Minor| -> Result<u64> {
            amount
                .at_scale(decimals)
                .and_then(Minor::unsigned)
                .map_err(|_| bad("Invalid total"))
        };
        total = verified_kernel::amount_sum(total, minor(calc.total_price)?);
        taxes = verified_kernel::amount_sum(taxes, minor(calc.tax)?);
        let discount_percent = one
            .checked_sub(discount)
            .and_then(|d| d.checked_mul(hundred))
            .and_then(|d| d.rounded(0))
            .map_err(bad)?;
        lines.push(json!({"id":p.id,"referencedId":p.id,"productNumber":p.product_number,"parentId":p.parent_id,"media":p.media,"label":format!("{}{}",p.name,p.options.as_object().filter(|o|!o.is_empty()).map(|o|format!(" · {}",o.values().filter_map(|v|v.as_str()).collect::<Vec<_>>().join(" / "))).unwrap_or_default()),"quantity":i.quantity,"stock":p.stock,"price":{"unitPrice":calc.unit_price,"totalPrice":calc.total_price,"calculatedTaxes":calc.calculated_taxes.iter().map(|t|json!({"tax":t.tax,"taxRate":t.tax_rate,"price":t.price})).collect::<Vec<_>>(),"listPrice":calc.list_price,"regulationPrice":calc.regulation_price.map(|price|json!({"price":price})),"referencePrice":calc.reference_price},"discountPercent":discount_percent,"ruleId":tier.map(|t|&t.rule_id),"minPurchase":p.min_purchase,"purchaseSteps":p.purchase_steps,"maxPurchase":p.max_purchase}));
    }
    for line in &mut lines {
        let mut configs = c
            .data
            .app_configurations
            .values()
            .filter(|v| Some(v.product_id.as_str()) == view::<LineView>(line).id.as_str())
            .map(|v| json!(v))
            .collect::<Vec<_>>();
        configs.sort_by(|a, b| {
            let (a, b) = (view::<ConfigApp>(a).app, view::<ConfigApp>(b).app);
            a.as_str().cmp(&b.as_str())
        });
        if let Some(first) = configs.first() {
            merge(
                line,
                LineConfigurations {
                    configuration: first,
                    app_configurations: &configs,
                },
            )?;
        }
    }
    let amount = |minor: u64| Minor::from_unsigned(minor, decimals).map_err(bad);
    let payable = amount(if b2b {
        verified_kernel::amount_sum(total, taxes)
    } else {
        total
    })?;
    let net = amount(if b2b {
        total
    } else {
        verified_kernel::net_amount(total, taxes)
    })?;
    let total = amount(total)?;
    let taxes = amount(taxes)?;
    Ok(
        json!({"token":c.token,"id":c.id,"revision":c.revision,"status":c.status,"lineItems":lines,"customerGroup":c.data.group,"customerId":c.data.customer_id,"customerEmail":c.data.email,"company":c.data.company,"price":{"positionPrice":total,"totalPrice":payable,"netPrice":net,"tax":taxes,"taxStatus":if b2b{"net"}else{"gross"},"currency":currency.code,"currencyScale":currency.scale},"currencyContext":currency_context(cfg,currency),"availableCurrencies":cfg.enabled,"order":c.data.order}),
    )
}
/// The fee of an app configuration in the cart currency.
pub fn configuration_price(
    config: &AppConfiguration,
    cfg: &Config,
    currency: &Definition,
) -> Result<Decimal> {
    if let Some(source) = &config.currency {
        let scale = scale(source).ok_or(bad("Configuration currency unavailable"))?;
        convert_from(
            Decimal::new(config.fee_minor.into(), scale.into()).map_err(bad)?,
            source,
            cfg,
            currency,
        )
    } else {
        convert(
            Decimal::new(config.fee_minor.into(), 2).map_err(bad)?,
            cfg,
            currency,
        )
    }
}
/// The nearest quantity the product allows: its minimum, steps and a maximum of 10,000.
pub fn normalized_quantity(p: &Product, q: u32) -> Result<u32> {
    let available = p.max_purchase.unwrap_or(10000).min(10000);
    if available < p.min_purchase {
        return Err(bad("Product cannot be purchased in an allowed quantity"));
    }
    Ok(context::fix_quantity(
        p.min_purchase as i64,
        q.max(p.min_purchase).min(available) as i64,
        p.purchase_steps as i64,
    ) as u32)
}

#[cfg(test)]
mod tests {
    use super::{Decimal, configuration_price, normalized_quantity, quote};
    use serde_json::{Value, json};
    use veyndo_pricing::catalog::Product;
    use veyndo_shop::Settings;
    use veyndo_shop::types::cart::{AppConfiguration, StoredCart};

    fn product(min: u32, steps: u32, max: Option<u32>) -> Product {
        serde_json::from_value(json!({
            "extra":{},"id":"p1","name":"Lamp","category":"","description":"","price":119.0,
            "tax_rate":19.0,"stock":5,"revision":1,"list_price":null,"regulation_price":null,
            "reference_price":null,"advanced_prices":[],"min_purchase":min,"purchase_steps":steps,
            "max_purchase":max,"parent_id":null,"options":{},"media":[],"properties":{},
            "delivery_days":1
        }))
        .unwrap()
    }

    fn settings() -> Settings {
        serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap()
    }

    fn cart(items: Value) -> StoredCart {
        StoredCart {
            id: "c1".into(),
            tenant: "demo".into(),
            token: "t".into(),
            data: serde_json::from_value(json!({"items":items,"group":"consumer","email":null,"company":null,"session":"s","buyer":null,"order":null})).unwrap(),
            revision: 3,
            status: "open".into(),
        }
    }

    #[test]
    fn quantities_follow_the_minimum_steps_and_maximum() {
        assert_eq!(normalized_quantity(&product(1, 1, None), 3).unwrap(), 3);
        assert_eq!(normalized_quantity(&product(2, 2, None), 1).unwrap(), 2);
        assert_eq!(normalized_quantity(&product(2, 2, None), 5).unwrap(), 4);
        assert_eq!(normalized_quantity(&product(1, 1, Some(4)), 9).unwrap(), 4);
        assert!(normalized_quantity(&product(5, 1, Some(3)), 1).is_err());
    }

    #[test]
    fn a_consumer_quote_sums_gross_lines_and_refuses_unaligned_quantities() {
        let s = settings();
        let ps = [product(1, 1, None)];
        let q = quote(&cart(json!([{"id":"p1","quantity":2}])), &ps, &s).unwrap();
        assert_eq!(q["revision"], 3);
        assert_eq!(q["lineItems"][0]["quantity"], 2);
        assert_eq!(q["price"]["taxStatus"], "gross");
        assert_eq!(q["price"]["positionPrice"], q["price"]["totalPrice"]);
        assert!(quote(&cart(json!([{"id":"missing","quantity":1}])), &ps, &s).is_err());
        let stepped = [product(2, 2, None)];
        assert!(quote(&cart(json!([{"id":"p1","quantity":3}])), &stepped, &s).is_err());
    }

    #[test]
    fn a_configuration_fee_without_currency_is_read_in_cents() {
        let s = settings();
        let currency = s.currencies.selected("").unwrap();
        let config: AppConfiguration = serde_json::from_value(json!({"app":"engraving","productId":"p1","fields":{},"feeMinor":250,"ruleRevision":1,"appVersion":"1.0.0","entity":"engravings","record":"r1"})).unwrap();
        let fee = configuration_price(&config, &s.currencies, currency).unwrap();
        assert!(fee > Decimal::ZERO);
    }
}
