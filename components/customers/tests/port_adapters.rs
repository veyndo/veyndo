//! Test adapter for the customers port `SignIn`, so the customers mirror builds and tests without the server.
use serde_json::{Value, json};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use veyndo_core::{PortFuture, RequestContext, Result, bad};
use veyndo_customers::ports::SignIn;
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

/// Refuses carts a POS cashier holds; prices nothing, it returns the line count.
struct OnlineSignIn;

impl SignIn for OnlineSignIn {
    fn cashier(&self, _: &RequestContext, cart: &StoredCart) -> Result<()> {
        if cart.data.pos_actor.is_some() {
            Err(bad("cart belongs to a register"))
        } else {
            Ok(())
        }
    }
    fn view<'a>(&'a self, cart: &'a StoredCart) -> PortFuture<'a, Value> {
        let view = json!({"id": cart.id, "items": cart.data.items.len()});
        Box::pin(async move { Ok(view) })
    }
}

fn cart(pos_actor: Option<&str>) -> StoredCart {
    StoredCart {
        id: "cart-1".into(),
        tenant: "shop-a".into(),
        token: "token-1".into(),
        data: serde_json::from_value(json!({
            "items": [{"id": "a", "quantity": 1}], "group": "private", "email": null,
            "company": null, "session": "session-1", "buyer": null, "order": null,
            "pos_actor": pos_actor
        }))
        .unwrap(),
        revision: 1,
        status: "open".into(),
    }
}

#[test]
fn sign_in_port_guards_register_carts_and_views_the_cart() {
    let port: Arc<dyn SignIn> = Arc::new(OnlineSignIn);
    let request = RequestContext::new();
    assert!(port.cashier(&request, &cart(None)).is_ok());
    assert!(port.cashier(&request, &cart(Some("cashier-1"))).is_err());
    assert_eq!(ready(port.view(&cart(None))).unwrap()["items"], 1);
}
