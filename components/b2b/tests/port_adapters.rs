//! Test adapters for the b2b ports (`QuoteLifecycle`, `ListCarts`, `Procurement`), so the b2b mirror builds and tests without the server.
use serde_json::{Value, json};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use veyndo_b2b::ports::{ListCarts, Procurement, QuoteLifecycle};
use veyndo_core::{PortFuture, RequestContext, Result, bad, conflict};
use veyndo_pricing::domain::cart::Item as Line;
use veyndo_shop::domain::configuration_selection::Selection;
use veyndo_shop::ports::Carts;
use veyndo_shop::types::cart::{Item, StoredCart};

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

/// No app declares a quote lifecycle.
struct NoLifecycle;

impl QuoteLifecycle for NoLifecycle {
    fn capture<'a>(&'a self, _: &'a mut sqlx::PgConnection, _: &'a str) -> PortFuture<'a, Value> {
        Box::pin(async { Ok(Value::Null) })
    }
}

/// Lists apply at revision 3 only and admit selections of the installed app `engraving`.
struct Lists;

impl ListCarts for Lists {
    fn check<'a>(&'a self, _: &'a str, selection: &'a Selection) -> PortFuture<'a, ()> {
        let answer = if selection.app == "engraving" {
            Ok(())
        } else {
            Err(bad("app not installed"))
        };
        Box::pin(async move { answer })
    }
    fn apply<'a>(
        &'a self,
        _: &'a RequestContext,
        items: Vec<Item>,
        expected: i64,
        _: &'a [Selection],
    ) -> PortFuture<'a, Value> {
        let answer = if expected == 3 {
            Ok(json!({"revision": 4, "items": items.len()}))
        } else {
            Err(conflict("cart revision changed"))
        };
        Box::pin(async move { answer })
    }
}

/// Carts are not part of these tests; every call is refused.
struct NoCarts;

impl Carts for NoCarts {
    fn load<'a>(&'a self, _: &'a RequestContext) -> PortFuture<'a, StoredCart> {
        Box::pin(async { Err(bad("no cart in this test")) })
    }
    fn view<'a>(&'a self, _: &'a StoredCart) -> PortFuture<'a, Value> {
        Box::pin(async { Err(bad("no cart in this test")) })
    }
    fn admit<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a RequestContext,
        _: &'a StoredCart,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Err(bad("no cart in this test")) })
    }
    fn replace<'a>(
        &'a self,
        _: &'a RequestContext,
        _: Vec<Line>,
        _: Option<i64>,
    ) -> PortFuture<'a, StoredCart> {
        Box::pin(async { Err(bad("no cart in this test")) })
    }
}

/// The company gateway's server side, composed from the adapters above.
struct Gateway;

impl Procurement for Gateway {
    fn carts(&self) -> &dyn Carts {
        &NoCarts
    }
    fn lists(&self) -> &dyn ListCarts {
        &Lists
    }
    fn lifecycle(&self) -> &dyn QuoteLifecycle {
        &NoLifecycle
    }
    fn customer<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, ()> {
        let answer = if h.principal.user.is_some() {
            Ok(())
        } else {
            Err(bad("customer session required"))
        };
        Box::pin(async move { answer })
    }
    fn order(&self, raw: &Value, created: &str) -> Value {
        json!({"id": raw["id"], "created": created})
    }
    fn labels<'a>(&'a self, _: &'a str) -> PortFuture<'a, Value> {
        Box::pin(async { Ok(json!({"open": "Open"})) })
    }
    fn cashier(&self, _: &RequestContext, cart: &StoredCart) -> Result<()> {
        if cart.data.pos_actor.is_some() {
            Err(bad("cart belongs to a register"))
        } else {
            Ok(())
        }
    }
    fn locked<'a, 'c: 'a>(
        &'a self,
        _: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _: &'a str,
        _: &'a str,
    ) -> PortFuture<'a, (veyndo_shop::Settings, i64)> {
        Box::pin(async { Err(bad("no settings in this test")) })
    }
    fn save<'a>(&'a self, _: &'a RequestContext, v: Value) -> PortFuture<'a, Value> {
        Box::pin(async move { Ok(v) })
    }
    fn companies<'a>(&'a self, _: &'a RequestContext, rows: &'a mut [Value]) -> PortFuture<'a, ()> {
        for row in rows.iter_mut() {
            row["extensions"] = json!({});
        }
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn procurement_ports_answer_without_the_server() {
    let gateway: Arc<dyn Procurement> = Arc::new(Gateway);
    let request = RequestContext::new();
    let selection = Selection {
        app: "engraving".into(),
        product_id: "ring".into(),
        fields: json!({}),
    };
    assert!(ready(gateway.lists().check("shop-a", &selection)).is_ok());
    let unknown = Selection {
        app: "unknown".into(),
        ..selection
    };
    assert!(ready(gateway.lists().check("shop-a", &unknown)).is_err());
    let lines = vec![Item {
        id: "ring".into(),
        quantity: 1,
    }];
    assert!(ready(gateway.lists().apply(&request, lines.clone(), 2, &[])).is_err());
    assert_eq!(
        ready(gateway.lists().apply(&request, lines, 3, &[])).unwrap()["revision"],
        4
    );
    assert!(
        ready(gateway.customer(&request)).is_err(),
        "an anonymous request has no customer session"
    );
    assert_eq!(
        gateway.order(&json!({"id": "o-1"}), "2026-11-25")["id"],
        "o-1"
    );
    let mut rows = vec![json!({"id": "c-1"})];
    ready(gateway.companies(&request, &mut rows)).unwrap();
    assert!(rows[0]["extensions"].is_object());
    assert!(ready(gateway.carts().load(&request)).is_err());
}
