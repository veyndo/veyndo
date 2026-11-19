//! Destination tax-class resolution and net-preserving price conversion.
use serde_json::json;
use std::collections::HashMap;
use veyndo_core::{Result, bad};
use veyndo_pricing::catalog::Product;
use veyndo_pricing::service::conversion::amount;
use veyndo_shop::domain::tax_rules::destination_rate;
use veyndo_shop::types::settings::{CheckoutSelection, Settings};

pub fn tax_products(ps: &[Product], c: &CheckoutSelection, s: &Settings) -> Result<Vec<Product>> {
    if !s.countries.contains(&c.country) {
        return Err(bad("Country is not supported"));
    }
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut rates = HashMap::new();
    ps.iter()
        .map(|p| {
            let mut p = p.clone();
            let tax = s
                .taxes
                .iter()
                .find(|t| {
                    t.id == if let Some(id) = p.extra["taxClassId"].as_str() {
                        id
                    } else if p.tax_rate == 7. {
                        "reduced"
                    } else {
                        "standard"
                    }
                })
                .ok_or(bad("Tax class missing"))?;
            let rate = if let Some(rate) = rates.get(&tax.id) {
                *rate
            } else {
                let rate = destination_rate(tax, c, &today)?;
                rates.insert(tax.id.clone(), rate);
                rate
            };
            let factor = (1. + rate / 100.) / (1. + p.tax_rate / 100.);
            if factor != 1.
                && let Some(prices) = p.extra["currencyPrices"].as_object_mut()
            {
                for (code, entry) in prices {
                    if let Some(d) = s.currencies.definitions.iter().find(|d| d.code == *code) {
                        for key in ["price", "listPrice", "regulationPrice"] {
                            if let Some(text) = entry[key].as_str() {
                                let value = text
                                    .parse::<f64>()
                                    .map_err(|_| bad("Invalid fixed currency price"))?;
                                entry[key] =
                                    json!(amount(value * factor, code, d.scale)?.decimal());
                            }
                        }
                    }
                }
            }
            p.price *= factor;
            p.list_price = p.list_price.map(|v| v * factor);
            p.regulation_price = p.regulation_price.map(|v| v * factor);
            p.tax_rate = rate;
            Ok(p)
        })
        .collect()
}
// Parents remain the six listing entries; actual child SKUs carry their own prices/stock.
