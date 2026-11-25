//! Project the current native cart quote for the independent checkout session service.
use crate::cart_storage::load_cart;
use crate::foundation::App;
use crate::{b2b, cart_price, marketing};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::CACHE_CONTROL;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_shop::types::cart::StoredCart;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct QuoteInput {
    cart_id: String,
    cart_revision: i64,
    customer_id: String,
}

/// The parts of the projected cart quote the session service receives; each passes on exactly as projected.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Quote {
    locale: Value,
    #[serde(deserialize_with = "lenient")]
    price: QuotePrice,
    #[serde(deserialize_with = "lenient")]
    checkout: QuoteCheckout,
    selection_needs_confirmation: Value,
    #[serde(deserialize_with = "lenient")]
    legal: QuoteLegal,
    #[serde(deserialize_with = "lenient")]
    payment_method: QuotePayment,
}

/// The quote total.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuotePrice {
    currency: Value,
    total_minor: Value,
    currency_scale: Value,
}

/// The checkout destination of the quote.
#[derive(Deserialize, Default)]
#[serde(default)]
struct QuoteCheckout {
    country: Value,
}

/// The legal requirements of the quote.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct QuoteLegal {
    strict_checkout: Value,
    requires_digital: Value,
    policy_version: Value,
}

/// The selected payment method of the quote.
#[derive(Deserialize, Default)]
#[serde(default)]
struct QuotePayment {
    id: Value,
    mode: Value,
}

async fn owned_cart(
    a: &App,
    h: &RequestContext,
    cart_id: &str,
    customer_id: &str,
) -> Result<StoredCart> {
    let c = load_cart(a, h).await?;
    if c.id != cart_id || c.data.customer_id.as_deref() != Some(customer_id) {
        return Err(Error(StatusCode::NOT_FOUND, "Cart unavailable".into()));
    }
    b2b::surface_context(a, h, "customerId", customer_id).await?;
    if c.data.admin_actor.is_some()
        || c.data.pos_actor.is_some()
        || veyndo_core::store::environments::parent(a, &c.tenant)
            .await?
            .is_some()
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "A live customer cart is required".into(),
        ));
    }
    Ok(c)
}

pub(crate) async fn quote_session(
    State(a): State<App>,
    mut h: RequestContext,
    Json(input): Json<QuoteInput>,
) -> Result<impl IntoResponse> {
    super::buyer_identity::bind(&mut h)?;
    let c = owned_cart(&a, &h, &input.cart_id, &input.customer_id).await?;
    if c.status != "open" || c.revision != input.cart_revision || c.data.items.is_empty() {
        return Err(conflict("Open and review the current nonempty cart"));
    }
    let ids = c
        .data
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    for batch in ids.chunks(24) {
        marketing::admit_products(&a, &h, batch).await?;
    }
    let q: Quote = view(&cart_price::project_cart(&a, &c, true).await?);
    Ok((
        [(CACHE_CONTROL, "no-store")],
        Json(json!({
            "tenant": c.tenant,
            "customerId": input.customer_id,
            "cartId": c.id,
            "cartRevision": c.revision,
            "salesChannel": c.data.sales_channel,
            "environment": "live",
            "locale": q.locale,
            "currency": q.price.currency,
            "country": q.checkout.country,
            "totalMinor": q.price.total_minor,
            "currencyScale": q.price.currency_scale,
            "selectionNeedsConfirmation": q.selection_needs_confirmation,
            "legal": {
                "strictCheckout":q.legal.strict_checkout,
                "requiresDigital":q.legal.requires_digital,
            },
            "legalPolicyVersion": q.legal.policy_version,
            "paymentMethod": q.payment_method.id,
            "paymentMode": q.payment_method.mode,
        })),
    ))
}
