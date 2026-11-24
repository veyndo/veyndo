//! Independent customer sessions, profile/password management and owning-account order history.
use crate::{App, load_cart};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_core::shop_scope::tenant;
mod payment_preference;
pub(crate) use payment_preference::remember_payment;
mod address_store;
pub(crate) mod browser;
mod demo;
mod orders;
pub(crate) use address_store::*;
pub(crate) use demo::seed_address as seed_demo_address;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .route(
            "/store-api/account/register",
            post(veyndo_customers::http::account::register),
        )
        .route(
            "/store-api/account/addresses",
            get(veyndo_customers::http::addresses::list)
                .post(veyndo_customers::http::addresses::create),
        )
        .route(
            "/store-api/account/addresses/{id}",
            axum::routing::put(veyndo_customers::http::addresses::save)
                .delete(veyndo_customers::http::addresses::remove),
        )
        .route(
            "/store-api/account/profile",
            get(veyndo_customers::http::account::get).put(veyndo_customers::http::account::save),
        )
        .route("/store-api/account/orders", get(orders::list))
        .route("/store-api/account/orders/{id}", get(orders::detail))
        .route(
            "/store-api/account/orders/{order}/receipts/{id}/pdf",
            get(orders::receipt),
        )
        .route("/store-api/account/logout", post(logout))
        .route(
            "/store-api/account/password",
            post(veyndo_customers::http::account::password),
        )
}
pub(crate) use veyndo_core::shop_scope::session_digest;

pub(crate) async fn identity(a: &App, h: &RequestContext) -> Result<(String, String)> {
    let t = tenant(h)?;
    let digest = session_digest(h)?;
    let email = veyndo_customers::service::sessions::identify(&a.core, &t, &digest).await?;
    Ok((t, email))
}
async fn logout(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let (t, email) = identity(&a, &h).await?;
    if header(&h, "sw-context-token").is_some() {
        load_cart(&a, &h).await?;
    }
    veyndo_customers::service::sessions::close(
        &a.core,
        &t,
        header(&h, "x-customer-token").unwrap(),
    )
    .await?;
    if let Some(token) = header(&h, "sw-context-token") {
        veyndo_checkout::store::cart_load::forget_customer(&a, &t, token, &email).await?;
    }
    Ok(Json(json!({"loggedOut":true})))
}

pub(crate) use orders::project as project_order;
