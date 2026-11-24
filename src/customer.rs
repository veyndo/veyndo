//! Buyer sign-in route and the server side of its port; the customers component owns credentials, cart binding and rotation.
use crate::{App, cart_json, pos};
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_shop::types::cart::StoredCart;

pub(crate) async fn login(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_customers::service::sign_in::login(&SignIn(&a), &a, &h, &v).await?,
    ))
}

/// The cashier guard and priced cart view of this process.
struct SignIn<'s>(&'s App);
impl veyndo_customers::ports::SignIn for SignIn<'_> {
    fn cashier(&self, h: &RequestContext, cart: &StoredCart) -> Result<()> {
        pos::guard_cart(h, cart)
    }
    fn view<'a>(&'a self, cart: &'a StoredCart) -> veyndo_core::PortFuture<'a, Value> {
        Box::pin(cart_json(self.0, cart))
    }
}
