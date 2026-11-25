//! Test adapters for the checkout ports (`Quotes`, `CartEdits`, `Purchase`), so the checkout mirror builds and tests without the server.
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use veyndo_checkout::ports::{CartEdits, LockedQuote, Purchase, Quotes};
use veyndo_core::{PortFuture, RequestContext, Result, bad};
use veyndo_pricing::catalog::Product;
use veyndo_pricing::domain::cart::Item;
use veyndo_shop::Settings;
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

fn cart() -> StoredCart {
    StoredCart {
        id: "cart-1".into(),
        tenant: "shop-a".into(),
        token: "token-1".into(),
        data: serde_json::from_value(json!({
            "items": [{"id": "a", "quantity": 1}], "group": "private", "email": null,
            "company": null, "session": "session-1", "buyer": null, "order": null
        }))
        .unwrap(),
        revision: 1,
        status: "open".into(),
    }
}

type Tx<'a, 'c> = &'a mut sqlx::Transaction<'c, sqlx::Postgres>;

/// A shop without apps: no products are known, invoice is the only method, quotes pass unchanged.
struct PlainShop;

impl Quotes for PlainShop {
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
        Box::pin(async {
            Ok(HashSet::from([(
                "native".to_string(),
                "invoice".to_string(),
            )]))
        })
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

impl CartEdits for PlainShop {
    fn view<'a>(&'a self, cart: &'a StoredCart) -> PortFuture<'a, Value> {
        let view = json!({"id": cart.id, "revision": cart.revision});
        Box::pin(async move { Ok(view) })
    }
    fn configure<'a>(&'a self, cart: &'a mut StoredCart) -> PortFuture<'a, ()> {
        cart.data.app_configurations.clear();
        Box::pin(async { Ok(()) })
    }
    fn locked<'a, 'c: 'a>(&'a self, _: Tx<'a, 'c>, _: &'a StoredCart) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn committed<'a, 'c: 'a>(&'a self, _: Tx<'a, 'c>, _: &'a StoredCart) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

impl Purchase for PlainShop {
    type Prepared = ();
    fn prepare<'a>(&'a self, _: &'a StoredCart, business: bool) -> PortFuture<'a, ()> {
        let answer = if business {
            Err(bad("no business policy in this shop"))
        } else {
            Ok(())
        };
        Box::pin(async move { answer })
    }
    fn methods_locked<'a, 'c: 'a>(
        &'a self,
        _: Tx<'a, 'c>,
        _: &'a StoredCart,
        _: &'a str,
        settings: Settings,
    ) -> PortFuture<'a, Settings> {
        Box::pin(async move { Ok(settings) })
    }
    fn register_locked<'a, 'c: 'a>(
        &'a self,
        _: Tx<'a, 'c>,
        _: &'a RequestContext,
        _: &'a StoredCart,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn configurations_locked<'a, 'c: 'a>(
        &'a self,
        _: Tx<'a, 'c>,
        _: &'a StoredCart,
        _: &'a (),
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn adjust_locked<'a, 'c: 'a>(
        &'a self,
        _: Tx<'a, 'c>,
        _: LockedQuote<'a>,
        quote: Value,
        _: &'a (),
    ) -> PortFuture<'a, Value> {
        Box::pin(async move { Ok(quote) })
    }
    fn approve(&self, _: &(), _: &str, _: i64) -> Result<()> {
        Err(bad("no business policy in this shop"))
    }
    fn settlement(
        &self,
        _: &RequestContext,
        _: &StoredCart,
        _: &Value,
        _: &Settings,
        _: i64,
    ) -> Result<()> {
        Ok(())
    }
    fn provider_ready(&self, _: &str, payment_method: &str) -> Result<()> {
        if payment_method == "invoice" {
            Ok(())
        } else {
            Err(bad("provider not configured"))
        }
    }
    fn bind_order<'a, 'c: 'a>(
        &'a self,
        _: Tx<'a, 'c>,
        _: &'a str,
        _: &'a mut Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn settle<'a, 'c: 'a>(
        &'a self,
        _: Tx<'a, 'c>,
        _: &'a RequestContext,
        _: &'a StoredCart,
        _: &'a mut Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn prepare_payment<'a, 'c: 'a>(
        &'a self,
        _: Tx<'a, 'c>,
        _: &'a StoredCart,
        _: &'a mut Value,
        _: i64,
        _: Option<&'a str>,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn fence<'a, 'c: 'a>(&'a self, _: Tx<'a, 'c>) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn checkout_ports_answer_without_the_server() {
    let quotes: Arc<dyn Quotes> = Arc::new(PlainShop);
    assert!(
        ready(quotes.products("shop-a", &[], &[]))
            .unwrap()
            .is_empty()
    );
    let methods = ready(quotes.available("shop-a", "default", "EUR")).unwrap();
    assert!(methods.contains(&("native".to_string(), "invoice".to_string())));
    let edits: Arc<dyn CartEdits> = Arc::new(PlainShop);
    let mut stored = cart();
    ready(edits.configure(&mut stored)).unwrap();
    assert_eq!(ready(edits.view(&stored)).unwrap()["revision"], 1);
    let purchase = PlainShop;
    assert!(ready(purchase.prepare(&stored, false)).is_ok());
    assert!(ready(purchase.prepare(&stored, true)).is_err());
    assert!(purchase.provider_ready("shop-a", "invoice").is_ok());
    assert!(purchase.provider_ready("shop-a", "card").is_err());
    assert!(purchase.approve(&(), "digest", 100).is_err());
}

/// A shop without apps or experiments: an empty policy digest and no counted exposure.
impl veyndo_checkout::ports::PurchaseRecords for PlainShop {
    fn policy_digest<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Ok(String::new()) })
    }
    fn count_purchase<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
        _: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

/// No buyer session is known to this shop.
impl veyndo_payments::ports::Buyers for PlainShop {
    fn session_customer<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
        _: &'a str,
    ) -> PortFuture<'a, Option<String>> {
        Box::pin(async { Ok(None) })
    }
}
