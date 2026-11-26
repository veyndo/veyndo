//! Test adapter for the POS port `Register`, so the pos mirror builds and tests without the server.
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_checkout::ports::Quotes;
use veyndo_core::{PortFuture, RequestContext, Result, bad};
use veyndo_pos::ports::Register;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::domain::cart::Item;
use veyndo_shop::Settings;
use veyndo_shop::ports::Carts;
use veyndo_shop::types::cart::StoredCart;

/// Polls a test adapter's answer; adapters answer at once, without a runtime.
fn ready<T>(mut future: PortFuture<'_, T>) -> Result<T> {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a test adapter answers at once"),
    }
}

/// The register has no open cart in these tests.
struct NoCart;

impl Carts for NoCart {
    fn load<'a>(&'a self, _: &'a RequestContext) -> PortFuture<'a, StoredCart> {
        Box::pin(async { Err(bad("no open cart at this register")) })
    }
    fn view<'a>(&'a self, _: &'a StoredCart) -> PortFuture<'a, Value> {
        Box::pin(async { Ok(json!({})) })
    }
    fn admit<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a RequestContext,
        _: &'a StoredCart,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn replace<'a>(
        &'a self,
        _: &'a RequestContext,
        _: Vec<Item>,
        _: Option<i64>,
    ) -> PortFuture<'a, StoredCart> {
        Box::pin(async { Err(bad("no open cart at this register")) })
    }
}

/// Cash is the only method; quotes pass unchanged.
struct CashQuotes;

impl Quotes for CashQuotes {
    fn products<'a>(
        &'a self,
        _: &'a str,
        _: &'a [String],
        _: &'a [Item],
    ) -> PortFuture<'a, Vec<Product>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn methods<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        _: &'a str,
        settings: Settings,
    ) -> PortFuture<'a, (Settings, Option<String>)> {
        Box::pin(async move { Ok((settings, None)) })
    }
    fn available<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        _: &'a str,
    ) -> PortFuture<'a, HashSet<(String, String)>> {
        Box::pin(async { Ok(HashSet::from([("native".to_string(), "cash".to_string())])) })
    }
    fn adjust<'a>(
        &'a self,
        _: &'a StoredCart,
        _: &'a StoredCart,
        _: &'a [Product],
        quote: Value,
        _: &'a Settings,
        _: i64,
    ) -> PortFuture<'a, Value> {
        Box::pin(async move { Ok(quote) })
    }
}

/// A register with a fixed one-product catalogue that refuses to place orders.
struct FixtureRegister;

impl Register for FixtureRegister {
    fn carts(&self) -> &dyn Carts {
        &NoCart
    }
    fn quotes(&self) -> &dyn Quotes {
        &CashQuotes
    }
    fn currency<'a>(
        &'a self,
        _: &'a RequestContext,
        name: &'a str,
        _: &'a Value,
    ) -> PortFuture<'a, Value> {
        let answer = match name {
            "currency.list" => Ok(json!(["EUR"])),
            other => Err(bad(format!("unknown capability {other}"))),
        };
        Box::pin(async move { answer })
    }
    fn select<'a>(&'a self, _: &'a RequestContext, v: Value) -> PortFuture<'a, Value> {
        Box::pin(async move { Ok(v) })
    }
    fn checkout<'a>(&'a self, _: &'a RequestContext, _: &'a str) -> PortFuture<'a, Value> {
        Box::pin(async { Err(bad("no open cart at this register")) })
    }
    fn catalog<'a>(
        &'a self,
        _: &'a RequestContext,
        criteria: CatalogCriteria,
    ) -> PortFuture<'a, Value> {
        let limit = criteria.limit.unwrap_or(20);
        Box::pin(async move { Ok(json!({"limit": limit, "items": [{"id": "coffee"}]})) })
    }
    fn detail<'a>(
        &'a self,
        _: &'a RequestContext,
        id: String,
        _: CatalogCriteria,
    ) -> PortFuture<'a, Value> {
        Box::pin(async move {
            if id == "coffee" {
                Ok(json!({"id": id}))
            } else {
                Err(bad("unknown product"))
            }
        })
    }
    fn receipt<'a>(
        &'a self,
        _: &'a RequestContext,
        order: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Value> {
        let answer = json!({"order": order, "receipt": id});
        Box::pin(async move { Ok(answer) })
    }
}

#[test]
fn register_port_answers_without_the_server() {
    let register: Arc<dyn Register> = Arc::new(FixtureRegister);
    let request = RequestContext::new();
    assert!(ready(register.carts().load(&request)).is_err());
    let methods = ready(register.quotes().available("shop-a", "pos", "EUR")).unwrap();
    assert!(methods.contains(&("native".to_string(), "cash".to_string())));
    assert_eq!(
        ready(register.currency(&request, "currency.list", &json!({}))).unwrap()[0],
        "EUR"
    );
    assert!(ready(register.currency(&request, "currency.drop", &json!({}))).is_err());
    let page = ready(register.catalog(
        &request,
        CatalogCriteria {
            limit: Some(5),
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(page["limit"], 5);
    assert!(ready(register.detail(&request, "tea".into(), CatalogCriteria::default())).is_err());
    assert!(ready(register.checkout(&request, "key-1")).is_err());
    assert_eq!(
        ready(register.receipt(&request, "o-1", "r-1")).unwrap()["receipt"],
        "r-1"
    );
}
