//! Single-use checkout transfer for independent storefronts; no app-specific catalog or checkout rules.
use crate::domain::quote_view::OrderRef;
use crate::store::cart_load;
use crate::store::carts::{lock_by_token, rotate_token, rotate_token_returning};
use crate::store::handoffs::{self, RecoveryKey, Ticket};
use crate::store::hosted_parent::registered_parent;
use axum::Json;
use axum::extract::State as Shared;
use axum::http::header::CACHE_CONTROL;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_b2b::store::access::guard_conn;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{channel, tenant, token};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, uid};
use veyndo_shop::domain::cart_admission::request;
use veyndo_shop::types::cart::StoredCart;

/// The JSON fields `issue` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct IssueInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    parent_origin: Option<String>,
}

/// `POST /store-api/checkout/handoff`: a ten-minute single-use ticket for the hosted checkout, with an optional recovery key for a framing storefront.
pub async fn issue(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: IssueInput = view(&v);
    let t = tenant(&h)?;
    let _ = cart_load::load(&a, &h).await?;
    let mut tx = a.db.begin().await?;
    let c = lock_by_token(&mut tx, &t, token(&h)?)
        .await?
        .ok_or(bad("Cart unavailable"))?
        .into_cart()?;
    request(&h, &c)?;
    guard_conn(&mut tx, &h, &c).await?;
    if c.status != "open" || c.data.items.is_empty() || input.revision != Some(c.revision) {
        return Err(conflict("Cart changed, empty or terminal"));
    }
    let ticket = uid();
    let parent = input.parent_origin.as_deref();
    let recovery = if let Some(origin) = parent {
        registered_parent(&mut tx, &t, &c.data.sales_channel, origin).await?;
        Some(uid())
    } else {
        None
    };
    handoffs::purge_expired(&mut tx, &t).await?;
    let recovery_digest = recovery.as_deref().map(hash);
    handoffs::issue(
        &mut tx,
        &Ticket {
            digest: &hash(&ticket),
            tenant: &t,
            cart: &c.id,
            recovery_digest: recovery_digest.as_deref(),
            recovery_origin: parent,
        },
    )
    .await?;
    tx.commit().await?;
    let mut result =
        json!({"checkoutPath":checkout_path(&c, &ticket),"expiresInSeconds":600,"singleUse":true});
    if let Some(key) = recovery {
        result["recovery"] = json!({"cartId":c.id,"key":key});
    }
    Ok(Json(result))
}

/// The hosted checkout path; the ticket stays in the fragment and never reaches a server log.
fn checkout_path(c: &StoredCart, ticket: &str) -> String {
    format!(
        "/checkout?shop={}&channel={}&language={}#checkout/{ticket}",
        c.tenant, c.data.sales_channel, c.data.locale
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// A recovery request of the framing storefront.
pub struct Recovery {
    cart_id: String,
    key: String,
    parent_origin: String,
    #[serde(default)]
    reopen: bool,
}

/// The one answer every refused recovery gets, so a key cannot be probed.
fn unavailable() -> Error {
    Error(
        StatusCode::NOT_FOUND,
        "Hosted checkout recovery unavailable".into(),
    )
}

/// `POST /store-api/checkout/handoff/recover`: the framing storefront polls or reopens its checkout with the recovery key.
pub async fn recover(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(input): Json<Recovery>,
) -> Result<impl IntoResponse + use<>> {
    let t = tenant(&h)?;
    let channel = channel(&h);
    if [&input.cart_id, &input.key]
        .iter()
        .any(|s| s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(unavailable());
    }
    let mut tx = a.db.begin().await?;
    registered_parent(&mut tx, &t, channel, &input.parent_origin).await?;
    let digest = hash(&input.key);
    // Status polling can read the last committed state while an order is being placed.
    let key = RecoveryKey {
        tenant: &t,
        cart: &input.cart_id,
        digest: &digest,
        origin: &input.parent_origin,
    };
    let c = handoffs::recovered_cart(&mut tx, &key, input.reopen)
        .await?
        .ok_or_else(unavailable)?
        .into_cart()?;
    if c.data.sales_channel != channel {
        return Err(unavailable());
    }
    if input.reopen {
        // The recovery key may have changed while this request waited for the cart lock.
        let key = RecoveryKey { cart: &c.id, ..key };
        if !handoffs::recovery_valid(&mut tx, &key).await? {
            return Err(unavailable());
        }
    }
    request(&h, &c)?;
    guard_conn(&mut tx, &h, &c).await?;
    let result = if c.status == "completed" {
        let order = c
            .data
            .order
            .as_ref()
            .map(view::<OrderRef>)
            .and_then(|o| o.id.as_str().map(str::to_owned))
            .ok_or_else(unavailable)?;
        json!({"state":"ordered","completion":{"cartId":c.id,"token":c.token,"orderId":order}})
    } else if c.status == "open" && input.reopen {
        let ticket = uid();
        // Fence the old frame before allowing a replacement ticket for this same cart.
        rotate_token(&mut tx, &t, &c.id, &uid()).await?;
        handoffs::reissue(&mut tx, &t, &c.id, &hash(&ticket)).await?;
        json!({"state":"open","checkoutPath":checkout_path(&c, &ticket)})
    } else if c.status == "open" {
        json!({"state":"open"})
    } else {
        return Err(unavailable());
    };
    tx.commit().await?;
    Ok(([(CACHE_CONTROL, "no-store")], Json(result)))
}
/// The JSON fields `consume` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ConsumeInput {
    #[serde(deserialize_with = "lenient")]
    ticket: Option<String>,
}

/// Redeems a ticket once: the cart gets a fresh token the hosted checkout continues with.
pub async fn consume(a: &State, h: &RequestContext, v: &Value) -> Result<StoredCart> {
    let input: ConsumeInput = view(v);
    let t = tenant(h)?;
    let ticket = input
        .ticket
        .as_deref()
        .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(bad("Invalid checkout ticket"))?;
    let mut tx = a.db.begin().await?;
    // Lock the cart first, matching issue/order lock order. The secret remains only in the browser fragment.
    let c = handoffs::lock_ticket_cart(&mut tx, &t, &hash(ticket))
        .await?
        .ok_or(Error(
            StatusCode::GONE,
            "Checkout ticket expired or already used".into(),
        ))?
        .into_cart()?;
    request(h, &c)?;
    guard_conn(&mut tx, h, &c).await?;
    if c.data.sales_channel != channel(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Cart belongs to another sales channel".into(),
        ));
    }
    if c.status != "open" {
        return Err(conflict("Cart is terminal"));
    }
    let consumed = handoffs::consume(&mut tx, &t, &hash(ticket)).await?;
    if consumed != 1 {
        return Err(Error(
            StatusCode::GONE,
            "Checkout ticket already used".into(),
        ));
    }
    let c = rotate_token_returning(&mut tx, &t, &c.id, &uid())
        .await?
        .into_cart()?;
    request(h, &c)?;
    guard_conn(&mut tx, h, &c).await?;
    tx.commit().await?;
    Ok(c)
}
