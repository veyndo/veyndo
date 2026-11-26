//! Test adapters for the shop ports (`Carts`, `SettingsGuards`), so the shop mirror builds and tests without the server.
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use veyndo_core::{PortFuture, RequestContext, Result, bad, conflict};
use veyndo_pricing::domain::cart::Item;
use veyndo_shop::ports::{Carts, SettingsGuards};
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

fn cart(items: Value) -> StoredCart {
    StoredCart {
        id: "cart-1".into(),
        tenant: "shop-a".into(),
        token: "token-1".into(),
        data: serde_json::from_value(json!({
            "items": items, "group": "private", "email": null, "company": null,
            "session": "session-1", "buyer": null, "order": null
        }))
        .unwrap(),
        revision: 1,
        status: "open".into(),
    }
}

/// One in-memory cart with the revision check the server's cart port performs.
struct MemoryCarts(Mutex<StoredCart>);

impl Carts for MemoryCarts {
    fn load<'a>(&'a self, _: &'a RequestContext) -> PortFuture<'a, StoredCart> {
        let cart = self.0.lock().unwrap().clone();
        Box::pin(async move { Ok(cart) })
    }
    fn view<'a>(&'a self, cart: &'a StoredCart) -> PortFuture<'a, Value> {
        let view =
            json!({"id": cart.id, "revision": cart.revision, "items": cart.data.items.len()});
        Box::pin(async move { Ok(view) })
    }
    fn admit<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a RequestContext,
        _: &'a StoredCart,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn replace<'a>(
        &'a self,
        _: &'a RequestContext,
        items: Vec<Item>,
        expected: Option<i64>,
    ) -> PortFuture<'a, StoredCart> {
        let mut stored = self.0.lock().unwrap();
        let answer = if expected.is_some_and(|revision| revision != stored.revision) {
            Err(conflict("cart revision changed"))
        } else {
            stored.data.items = items;
            stored.revision += 1;
            Ok(stored.clone())
        };
        Box::pin(async move { answer })
    }
}

/// Settings guards that admit object conditions and refuse everything else.
struct AdmitObjects;

impl SettingsGuards for AdmitObjects {
    fn condition(&self, v: &Value) -> Result<()> {
        if v.is_object() {
            Ok(())
        } else {
            Err(bad("condition must be an object"))
        }
    }
    fn lock<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn capture<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a str,
        _: &'a veyndo_shop::Settings,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn locale<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a str,
        _: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn cart_port_replaces_lines_only_at_the_expected_revision() {
    let carts: Arc<dyn Carts> = Arc::new(MemoryCarts(Mutex::new(cart(
        json!([{"id": "a", "quantity": 1}]),
    ))));
    let request = RequestContext::new();
    let loaded = ready(carts.load(&request)).unwrap();
    assert_eq!(ready(carts.view(&loaded)).unwrap()["items"], 1);
    let lines = vec![Item {
        id: "b".into(),
        quantity: 2,
    }];
    let stale = ready(carts.replace(&request, lines.clone(), Some(0)));
    assert!(
        stale.is_err(),
        "a stale revision must not replace the lines"
    );
    let replaced = ready(carts.replace(&request, lines, Some(1))).unwrap();
    assert_eq!(
        (replaced.revision, replaced.data.items[0].id.as_str()),
        (2, "b")
    );
}

#[test]
fn settings_guards_port_checks_conditions() {
    let guards: Arc<dyn SettingsGuards> = Arc::new(AdmitObjects);
    assert!(guards.condition(&json!({"field": "country"})).is_ok());
    assert!(guards.condition(&json!("country")).is_err());
}

/// No document of a component above pricing refers to a currency or bounds a price job.
impl veyndo_pricing::ports::References for AdmitObjects {
    fn product_ceiling<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a str,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Ok(String::new()) })
    }
    fn products_use<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a str,
        _: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
    fn carts_use<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a str,
        _: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
    fn units_use<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a str,
        _: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
}

/// No document refers to a shipping or payment method.
impl veyndo_shipping::ports::MethodReferences for AdmitObjects {
    fn orders<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
    fn carts<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
    fn overrides<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
    fn rules<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
    fn units<'a>(
        &'a self,
        _: &'a mut PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
}
