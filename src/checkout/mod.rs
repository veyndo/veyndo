//! Expose the commerce host contract through the existing native checkout transaction.
use crate::{App, RequestContext, Result, Value, accounts};
use axum::{Router, routing::post};

pub(crate) mod buyer_identity;
mod private_browser;
mod purchase;
mod session_order;
mod session_quote;

pub(crate) fn router() -> Router<App> {
    use crate::auth::SecureRoutes;
    Router::new()
        .merge(private_browser::router())
        .secure_route(
            "/api/identity/checkout/introspect",
            &[("POST", "public")],
            post(buyer_identity::inspect),
        )
        .route(
            "/store-api/checkout/session-access",
            post(buyer_identity::issue),
        )
        .route(
            "/store-api/checkout/session-order",
            post(session_order::confirm_session),
        )
        .route(
            "/store-api/checkout/session-receipt",
            post(session_order::recover_session),
        )
        .route(
            "/store-api/checkout/session-quote",
            post(session_quote::quote_session),
        )
}

pub(crate) async fn execute(
    a: &App,
    h: &RequestContext,
    key: &str,
    review: Option<&session_order::Review>,
) -> Result<Value> {
    use veyndo_checkout::service::purchase::Outcome;
    match veyndo_checkout::service::purchase::execute(a, &purchase::Server(a), h, key, review)
        .await?
    {
        Outcome::Replay(order) => Ok(order),
        Outcome::Placed(placed) => {
            accounts::remember_payment(a, &placed.cart, &placed.method).await;
            Ok(placed.order)
        }
    }
}
