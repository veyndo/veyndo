//! Destination tax-class resolution and net-preserving price conversion.
use crate::domain::quote_view::flags;
use serde_json::json;
use std::collections::HashMap;
use veyndo_core::document_slot::field;
use veyndo_core::{Result, bad};
use veyndo_pricing::binary::Binary;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::decimal::Decimal;
use veyndo_pricing::service::conversion::exact_amount;
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
            let class = flags(&p.extra).tax_class_id;
            let tax = s
                .taxes
                .iter()
                .find(|t| {
                    t.id == if let Some(id) = class.as_str() {
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
            // The net-preserving factor `(1 + rate / 100) / (1 + product rate / 100)`, applied with
            // the original binary arithmetic so every stored price changes exactly as before.
            let one = Binary::integer(1);
            let hundred = Binary::integer(100);
            let gross_of =
                |rate: Binary| rate.checked_div(hundred).and_then(|r| one.checked_add(r));
            let rate_binary = Binary::from_decimal(rate).map_err(bad)?;
            let product_rate = Binary::from_wire(p.tax_rate).map_err(bad)?;
            let factor = gross_of(rate_binary)
                .and_then(|g| g.checked_div(gross_of(product_rate)?))
                .map_err(bad)?;
            let scaled = |value: Binary| value.checked_mul(factor).map_err(bad);
            if factor != one
                && let Some(prices) = field(&mut p.extra, "currencyPrices")?.as_object_mut()
            {
                for (code, entry) in prices {
                    if let Some(d) = s.currencies.definitions.iter().find(|d| d.code == *code) {
                        for key in ["price", "listPrice", "regulationPrice"] {
                            if let Some(text) = entry[key].as_str() {
                                let value = Decimal::parse(text)
                                    .and_then(Binary::from_decimal)
                                    .map_err(|_| bad("Invalid fixed currency price"))?;
                                let converted = scaled(value)?.shortest().map_err(bad)?;
                                entry[key] =
                                    json!(exact_amount(converted, code, d.scale)?.decimal());
                            }
                        }
                    }
                }
            }
            let stored = |value| -> Result<_> {
                Ok(scaled(Binary::from_wire(value).map_err(bad)?)?.to_wire())
            };
            p.price = stored(p.price)?;
            p.list_price = p.list_price.map(stored).transpose()?;
            p.regulation_price = p.regulation_price.map(stored).transpose()?;
            p.tax_rate = rate_binary.to_wire();
            Ok(p)
        })
        .collect()
}
// Parents remain the six listing entries; actual child SKUs carry their own prices/stock.

#[cfg(test)]
mod tests {
    use super::tax_products;
    use serde_json::{Value, json};
    use veyndo_pricing::catalog::Product;
    use veyndo_shop::types::settings::{CheckoutSelection, Settings};

    fn product(tax_rate: f64, extra: Value) -> Product {
        serde_json::from_value(json!({
            "extra":extra,"id":"p1","name":"Lamp","category":"","description":"","price":119.0,
            "tax_rate":tax_rate,"stock":1,"revision":1,"list_price":238.0,"regulation_price":null,
            "reference_price":null,"advanced_prices":[],"min_purchase":1,"purchase_steps":1,
            "max_purchase":null,"parent_id":null,"options":{},"media":[],"properties":{},
            "delivery_days":1
        }))
        .unwrap()
    }

    fn selection(country: &str) -> CheckoutSelection {
        let mut c = CheckoutSelection::defaults();
        c.country = country.into();
        c
    }

    #[test]
    fn the_destination_rate_replaces_the_product_rate_and_keeps_the_net_price() {
        let s: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        let ps = [
            product(19., json!({})),
            product(7., json!({})),
            product(19., json!({"taxClassId":"reduced"})),
        ];
        let home = tax_products(&ps, &selection("DE"), &s).unwrap();
        assert_eq!(
            home.iter()
                .map(|p| (p.price, p.tax_rate))
                .collect::<Vec<_>>(),
            [(119., 19.), (119., 7.), (119. * 1.07 / 1.19, 7.)]
        );
        let abroad = tax_products(&ps, &selection("AT"), &s).unwrap();
        assert_eq!(
            abroad.iter().map(|p| p.tax_rate).collect::<Vec<_>>(),
            [20., 10., 10.]
        );
        assert!((abroad[0].price - 120.).abs() < 1e-9);
        assert!((abroad[0].list_price.unwrap() - 240.).abs() < 1e-9);
        assert_eq!(abroad[0].regulation_price, None);
        assert!((abroad[1].price / 1.10 - 119. / 1.07).abs() < 1e-9);
    }

    #[test]
    fn an_unsupported_country_or_unknown_tax_class_is_refused() {
        let s: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        assert!(tax_products(&[product(19., json!({}))], &selection("IT"), &s).is_err());
        assert!(
            tax_products(
                &[product(19., json!({"taxClassId":"luxury"}))],
                &selection("DE"),
                &s
            )
            .is_err()
        );
        assert!(tax_products(&[], &selection("DE"), &s).is_ok_and(|ps| ps.is_empty()));
    }
}
