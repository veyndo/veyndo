//! UCP transport adapters reuse native cart, B2B, pricing, approval and checkout owners.
use crate::cart_mutation;
use crate::cart_price::cart_json;
use crate::cart_storage::load_cart;
use crate::foundation::App;
use crate::order_checkout::checkout;
use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
use veyndo_checkout::store::cart_create::new_cart_context;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::view;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_shop::domain::cart_lines::validate as validate_items;
use veyndo_shop::types::cart::Item;
use veyndo_shop::types::cart::StoredCart;
mod cart;
mod consent;
mod document;
mod messages;
mod negotiation;
mod profile;
mod readiness;
mod views;
pub(crate) use cart::CAPABILITY as UCP_CART;
pub(crate) use consent::CAPABILITY as UCP_CONSENT;
use document::{UcpDocument, ucp_document};
pub(crate) use negotiation::command as ucp_b2b;
pub(crate) use profile::{
    B2B as UCP_B2B, CHECKOUT as UCP_CHECKOUT, SCHEMA as UCP_B2B_SCHEMA, VERSION as UCP_B2B_VERSION,
};
pub(crate) use profile::{schema as ucp_b2b_schema, ucp_meta, ucp_profile};
pub(crate) use readiness::readiness;
pub(crate) use views::keep_present;
use views::{BuyerContact, LineInput, SessionInput};
pub(crate) const UCP_VERSION: &str = "2026-08-25";
pub(crate) fn ucp_items(v: &Value) -> Result<Vec<Item>> {
    let input: SessionInput = view(v);
    let rows = input
        .line_items
        .as_array()
        .ok_or(bad("line_items required"))?;
    let mut items = vec![];
    for i in rows {
        let line: LineInput = view(i);
        let q = line
            .quantity
            .filter(|q| *q <= 10000)
            .ok_or(bad("quantity required"))?;
        items.push(Item {
            id: line.item.id.ok_or(bad("item.id required"))?,
            quantity: q as u32,
        });
    }
    validate_items(&items)?;
    Ok(items)
}
pub(crate) async fn ucp_create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(create_session(a, h, v, false).await?.json()))
}
pub(super) async fn create_session(
    a: App,
    h: RequestContext,
    v: Value,
    cart_only: bool,
) -> Result<UcpDocument> {
    let selected = profile::select(&h, cart_only).await?;
    consent::admit(&v, selected.consent)?;
    let items = ucp_items(&v)?;
    let buyer = consent::buyer(&v, None)?;
    let input: SessionInput = view(&v);
    let mut h = h;
    if let Some(code) = input.currency.as_deref() {
        h.insert(
            "x-commerce-currency",
            code.parse().map_err(|_| bad("Invalid currency"))?,
        );
    }
    let c = new_cart_context(&a, &h, input.session.as_deref().unwrap_or(""), "ucp").await?;
    let mut ch = h.clone();
    ch.insert("sw-context-token", c.token.parse().unwrap());
    let privacy = consent::prepare(&a, &ch, &v).await?;
    let c = cart_mutation::set_consented_cart(&a, &ch, items, 1, buyer, privacy, None).await?;
    let mut doc = negotiation::response(&a, &ch, &c, selected).await?;
    doc.context_token = Some(c.token);
    Ok(doc)
}
pub(crate) async fn ucp_load(a: &App, h: &RequestContext, id: &str) -> Result<StoredCart> {
    let c = load_cart(a, h).await?;
    if c.id != id {
        return Err(Error(StatusCode::NOT_FOUND, "Checkout not found".into()));
    }
    Ok(c)
}
pub(crate) async fn ucp_get(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let selected = profile::select(&h, false).await?;
    let c = ucp_load(&a, &h, &id).await?;
    Ok(Json(
        negotiation::response(&a, &h, &c, selected).await?.json(),
    ))
}
pub(crate) async fn ucp_update(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: SessionInput = view(&v);
    let selected = profile::select(&h, false).await?;
    consent::admit(&v, selected.consent)?;
    let c = ucp_load(&a, &h, &id).await?;
    let buyer = consent::buyer(&v, c.data.buyer.as_ref())?;
    let privacy = consent::prepare(&a, &h, &v).await?;
    let c = cart_mutation::set_consented_cart(
        &a,
        &h,
        ucp_items(&v)?,
        c.revision,
        buyer,
        privacy,
        input.currency.as_deref(),
    )
    .await?;
    Ok(Json(
        negotiation::response(&a, &h, &c, selected).await?.json(),
    ))
}
pub(crate) async fn ucp_complete(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let selected = profile::select(&h, false).await?;
    consent::admit(&v, selected.consent)?;
    let mut current = ucp_load(&a, &h, &id).await?;
    if current.status == "open" && view::<SessionInput>(&v).buyer.is_some() {
        let buyer = consent::buyer(&v, current.data.buyer.as_ref())?;
        let privacy = consent::prepare(&a, &h, &v).await?;
        current = cart_mutation::set_consented_cart(
            &a,
            &h,
            current.data.items.clone(),
            current.revision,
            buyer,
            privacy,
            None,
        )
        .await?;
    }
    let mut projected = ucp_document(&current, &cart_json(&a, &current).await?);
    consent::project(&a, &h, &current, &mut projected).await?;
    if current.status == "open"
        && consent::missing(projected.buyer.as_ref().unwrap_or(&Value::Null))
    {
        return Err(conflict(
            "missing_consent_data: provide the contact required for the explicit consent",
        ));
    }
    if current.status == "open" && !BuyerContact::has_email(current.data.buyer.as_ref()) {
        return Err(conflict("Buyer email is required before demo completion"));
    }
    checkout(
        &a,
        &h,
        header(&h, "idempotency-key").ok_or(bad("Idempotency-Key required"))?,
    )
    .await?;
    let c = load_cart(&a, &h).await?;
    Ok(Json(
        negotiation::response(&a, &h, &c, selected).await?.json(),
    ))
}
pub(crate) async fn ucp_cancel(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let selected = profile::select(&h, false).await?;
    let c = ucp_load(&a, &h, &id).await?;
    let n = veyndo_checkout::store::carts::cancel_open(&a.core, &c.id).await?;
    if n != 1 && c.status != "cancelled" {
        return Err(conflict("Completed checkout cannot be cancelled"));
    }
    let c = load_cart(&a, &h).await?;
    Ok(Json(
        negotiation::response(&a, &h, &c, selected).await?.json(),
    ))
}

/// Routes remain under the shared tenant, channel, admission and authentication middleware.
pub(crate) fn ucp_router() -> Router<App> {
    Router::new()
        .route("/.well-known/ucp", get(ucp_profile))
        .route("/ucp/v1/carts", post(cart::create))
        .route("/ucp/2026-10-13/b2b-negotiation.json", get(ucp_b2b_schema))
        .route(
            "/ucp/v1/checkout-sessions/{id}/b2b/{operation}",
            post(ucp_b2b),
        )
        .route("/ucp/v1/checkout-sessions", post(ucp_create))
        .route(
            "/ucp/v1/checkout-sessions/{id}",
            get(ucp_get).put(ucp_update),
        )
        .route(
            "/ucp/v1/checkout-sessions/{id}/complete",
            post(ucp_complete),
        )
        .route("/ucp/v1/checkout-sessions/{id}/cancel", post(ucp_cancel))
}
