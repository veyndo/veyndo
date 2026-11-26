//! The commerce fields of a product as the editor sends them; stock is optional so an untouched quantity is never written back.
use crate::domain::media;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use veyndo_core::{Result, bad};
use veyndo_pricing::catalog::Tier;
use veyndo_pricing::decimal::Decimal;
use veyndo_pricing::pricing::ReferenceDefinition;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductFields {
    pub price: f64,
    pub tax_rate: f64,
    /// `None` keeps the stored quantity; `Some` is an explicit absolute quantity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<i32>,
    pub min_purchase: u32,
    pub purchase_steps: u32,
    pub max_purchase: Option<u32>,
    pub delivery_days: i32,
    pub list_price: Option<f64>,
    #[serde(default)]
    pub regulation_price: Option<f64>,
    #[serde(default)]
    pub reference_price: Option<ReferenceDefinition>,
    pub advanced_prices: Vec<Tier>,
    pub media: Value,
    pub properties: HashMap<String, String>,
}

impl ProductFields {
    /// Bounds on prices, purchase quantities, tiers, properties and media, independent of what is stored.
    pub fn validate(&self) -> Result<()> {
        if !self.price.is_finite()
            || !(0.0..=1_000_000.).contains(&self.price)
            || !self.tax_rate.is_finite()
            || !(0.0..=50.).contains(&self.tax_rate)
            || self.stock.is_some_and(|stock| stock < 0)
            || self.min_purchase == 0
            || self.min_purchase > 1_000_000
            || self.purchase_steps == 0
            || self.purchase_steps > 1_000_000
            || self
                .max_purchase
                .is_some_and(|v| v < self.min_purchase || v > 1_000_000)
            || !(0..=365).contains(&self.delivery_days)
            || self
                .list_price
                .is_some_and(|v| !v.is_finite() || !(0. ..=1_000_000.).contains(&v))
            || self.advanced_prices.len() > 100
            || self.properties.len() > 40
            || self
                .properties
                .iter()
                .any(|(k, v)| k.is_empty() || k.len() > 100 || v.len() > 1000)
        {
            return Err(bad("Invalid product commerce fields"));
        }
        if self
            .regulation_price
            .is_some_and(|n| !n.is_finite() || !(0.0..=1_000_000.0).contains(&n))
            || self.reference_price.as_ref().is_some_and(|p| {
                // Reference units are exact decimals: always finite once read.
                p.purchase_unit <= Decimal::ZERO
                    || p.reference_unit <= Decimal::ZERO
                    || p.unit_name.is_empty()
                    || p.unit_name.len() > 80
            })
        {
            return Err(bad("Invalid regulation or reference price"));
        }
        for tier in &self.advanced_prices {
            if tier.quantity_start == 0
                || tier.quantity_end.is_some_and(|n| n < tier.quantity_start)
                || !tier.discount.is_finite()
                || !(0.0..=1.).contains(&tier.discount)
                || tier.rule_id.len() > 100
            {
                return Err(bad("Invalid price tier"));
            }
        }
        media::validate(&self.media)
    }
}

#[cfg(test)]
mod tests {
    use super::ProductFields;
    use serde_json::{Value, json};

    fn fields(change: impl FnOnce(&mut Value)) -> ProductFields {
        let mut value = json!({
            "price":10.0,"taxRate":19.0,"minPurchase":1,"purchaseSteps":1,"maxPurchase":null,
            "deliveryDays":2,"listPrice":null,"advancedPrices":[],"media":[],"properties":{}
        });
        change(&mut value);
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn an_untouched_stock_is_absent_and_not_serialized() {
        let plain = fields(|_| {});
        assert!(plain.stock.is_none());
        assert!(plain.validate().is_ok());
        assert!(serde_json::to_value(&plain).unwrap().get("stock").is_none());
        assert_eq!(fields(|v| v["stock"] = json!(4)).stock, Some(4));
    }

    #[test]
    fn bounds_are_enforced() {
        for change in [
            json!({"price":-1.0}),
            json!({"taxRate":51.0}),
            json!({"stock":-1}),
            json!({"minPurchase":0}),
            json!({"minPurchase":5,"maxPurchase":4}),
            json!({"deliveryDays":366}),
            json!({"regulationPrice":-0.01}),
            json!({"referencePrice":{"purchase_unit":0.0,"reference_unit":1.0,"unit_name":"l"}}),
            json!({"advancedPrices":[{"rule_id":"r","quantity_start":0,"quantity_end":null,"discount":0.1}]}),
            json!({"advancedPrices":[{"rule_id":"r","quantity_start":2,"quantity_end":1,"discount":0.1}]}),
            json!({"media":[{"url":"http://x/a","id":"a","view":"v"}]}),
        ] {
            let candidate = fields(|v| {
                for (key, value) in change.as_object().unwrap() {
                    v[key] = value.clone();
                }
            });
            assert!(candidate.validate().is_err(), "{change}");
        }
    }
}
