//! Storefront currency discovery: the selected currency, the ones the channel offers and the catalogue of supported codes.
use crate::ports::Pricing;
use crate::service::conversion::context;
use crate::types::currency::{CODES, scale};
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use veyndo_core::shop_scope::{channel, header, tenant};
use veyndo_core::{RequestContext, Result};

pub async fn discover(State(p): State<Pricing>, h: RequestContext) -> Result<Json<Value>> {
    let (currencies, revision) = p.settings.currencies(&tenant(&h)?, channel(&h)).await?;
    let requested = if header(&h, "sw-context-token").is_some() {
        p.carts.currency(&h).await?
    } else {
        header(&h, "x-commerce-currency").unwrap_or("").into()
    };
    let d = currencies.selected(&requested)?;
    Ok(Json(
        json!({"currencyContext":context(&currencies,d),"availableCurrencies":currencies.definitions.iter().filter(|d|currencies.enabled.contains(&d.code)).collect::<Vec<_>>(),"defaultCurrency":currencies.default_currency,"baseCurrency":currencies.base_currency,"configuredCurrencies":currencies.definitions,"revision":revision,"catalogue":CODES.split(' ').map(|c|json!({"code":c,"scale":scale(c)})).collect::<Vec<_>>()}),
    ))
}

#[cfg(test)]
mod tests {
    use super::discover;
    use crate::ports::{Carts, PortFuture, Pricing, ShopSettings};
    use crate::types::currency::{Config, Definition};
    use axum::extract::State;
    use std::sync::Arc;
    use veyndo_core::RequestContext;

    struct Shop;
    impl ShopSettings for Shop {
        fn currencies<'a>(&'a self, _: &'a str, channel: &'a str) -> PortFuture<'a, (Config, i64)> {
            Box::pin(async move {
                let mut config = Config::default();
                config.definitions.push(Definition {
                    code: "USD".into(),
                    scale: 2,
                    rate: "1.25".into(),
                    strategy: "automatic".into(),
                });
                if channel == "us" {
                    config.enabled.push("USD".into());
                }
                Ok((config, 7))
            })
        }
    }
    struct Cart;
    impl Carts for Cart {
        fn currency<'a>(&'a self, _: &'a RequestContext) -> PortFuture<'a, String> {
            Box::pin(async { Ok("USD".to_owned()) })
        }
    }
    fn pricing() -> Pricing {
        Pricing {
            core: veyndo_core::State::detached(),
            settings: Arc::new(Shop),
            carts: Arc::new(Cart),
        }
    }
    fn request(pairs: &[(&'static str, &str)]) -> RequestContext {
        let mut h = RequestContext::new();
        h.validated_tenant = true;
        for (key, value) in pairs {
            h.insert(key, value.parse().unwrap());
        }
        h
    }

    #[tokio::test]
    async fn the_channel_decides_which_currencies_are_offered() {
        let base = [("x-tenant", "demo")];
        let answer = discover(State(pricing()), request(&base)).await.unwrap().0;
        assert_eq!(answer["currencyContext"]["code"], "EUR");
        assert_eq!(answer["availableCurrencies"].as_array().unwrap().len(), 1);
        assert_eq!(answer["configuredCurrencies"].as_array().unwrap().len(), 2);
        assert_eq!(answer["revision"], 7);
        // A currency the channel does not offer is refused, not silently replaced.
        let mut refused = base.to_vec();
        refused.push(("x-commerce-currency", "USD"));
        assert!(discover(State(pricing()), request(&refused)).await.is_err());
        let mut offered = refused.clone();
        offered.push(("sw-sales-channel-id", "us"));
        let answer = discover(State(pricing()), request(&offered))
            .await
            .unwrap()
            .0;
        assert_eq!(answer["currencyContext"]["code"], "USD");
    }

    #[tokio::test]
    async fn an_existing_cart_fixes_the_currency() {
        let h = request(&[
            ("x-tenant", "demo"),
            ("sw-sales-channel-id", "us"),
            ("sw-context-token", "t"),
        ]);
        let answer = discover(State(pricing()), h).await.unwrap().0;
        assert_eq!(answer["currencyContext"]["code"], "USD");
    }
}
