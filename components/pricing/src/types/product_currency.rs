//! The currency fields of a product's `extra` document: the currency its base prices are stated in and the fixed prices per currency as exact decimal strings.
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use veyndo_core::document::{lenient, view};

/// The currency fields of a product's `extra` document.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ProductCurrencies {
    #[serde(deserialize_with = "lenient")]
    pub price_currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    currency_prices: HashMap<String, Value>,
}

/// The fixed prices of one currency, as decimal strings.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct FixedPrices {
    #[serde(deserialize_with = "lenient")]
    price: Option<String>,
    #[serde(deserialize_with = "lenient")]
    list_price: Option<String>,
    #[serde(deserialize_with = "lenient")]
    regulation_price: Option<String>,
}

impl ProductCurrencies {
    pub fn of(extra: &Value) -> Self {
        view(extra)
    }

    /// The fixed prices stored for `code`; empty when there are none.
    pub fn fixed(&self, code: &str) -> FixedPrices {
        self.currency_prices.get(code).map(view).unwrap_or_default()
    }

    /// Whether anything (not null) is stored as fixed prices for `code`.
    pub fn has_fixed(&self, code: &str) -> bool {
        self.currency_prices.get(code).is_some_and(|v| !v.is_null())
    }
}

impl FixedPrices {
    /// A price by its stored field name (`price`, `listPrice`, `regulationPrice`).
    pub fn field(&self, name: &str) -> Option<&str> {
        match name {
            "price" => self.price.as_deref(),
            "listPrice" => self.list_price.as_deref(),
            "regulationPrice" => self.regulation_price.as_deref(),
            _ => None,
        }
    }
}
