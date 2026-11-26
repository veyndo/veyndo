//! Store API cart and order route adapters.
use crate::cart_mutation::set_items;
use crate::cart_price::cart_json;
use crate::cart_storage::load_cart;
use crate::foundation::App;
use crate::{cart_mutation, legal};
use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use veyndo_checkout::store::cart_create::new_cart_context;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
use veyndo_shop::types::cart::Item;

/// A storefront cart request: its session, items and the revision they were edited at.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CartRequest {
    #[serde(deserialize_with = "lenient")]
    session: Option<String>,
    items: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// One item added to the cart by product reference and quantity.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AddedItem {
    #[serde(deserialize_with = "lenient")]
    referenced_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    quantity: Option<u64>,
}

pub(crate) async fn create_cart(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let previous = if header(&h, "sw-context-token").is_some() {
        Some(load_cart(&a, &h).await?)
    } else {
        None
    };
    let c = new_cart_context(
        &a,
        &h,
        view::<CartRequest>(&v).session.as_deref().unwrap_or(""),
        "storefront",
    )
    .await?;
    if let Some(previous) = previous {
        legal::continue_consent(&a, &previous, &c).await?;
    }
    Ok(Json(cart_json(&a, &c).await?))
}
pub(crate) async fn get_cart(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let c = if header(&h, "sw-context-token").is_some() {
        load_cart(&a, &h).await?
    } else {
        new_cart_context(&a, &h, "", "storefront").await?
    };
    Ok(Json(cart_json(&a, &c).await?))
}
pub(crate) async fn edit_cart(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: CartRequest = view(&v);
    let items: Vec<Item> = serde_json::from_value(input.items).map_err(|e| bad(e.to_string()))?;
    let c = if let Some(review) = v.get("review") {
        let review = review
            .as_str()
            .filter(|s| s.len() == 64)
            .ok_or(bad("Invalid cart review"))?;
        let revision = input.revision.ok_or(bad("Cart revision required"))?;
        cart_mutation::reviewed_cart(&a, &h, items, revision, review).await?
    } else {
        set_items(&a, &h, items, input.revision).await?
    };
    Ok(Json(cart_json(&a, &c).await?))
}
pub(crate) async fn preview_cart(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: CartRequest = view(&v);
    let items: Vec<Item> = serde_json::from_value(input.items).map_err(|e| bad(e.to_string()))?;
    let revision = input.revision.ok_or(bad("Cart revision required"))?;
    Ok(Json(
        cart_mutation::preview_items(&a, &h, items, revision).await?,
    ))
}
pub(crate) async fn add_items(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    let mut items = c.data.items;
    let rows = view::<CartRequest>(&v)
        .items
        .as_array()
        .ok_or(bad("items array required"))?
        .iter()
        .map(view::<AddedItem>)
        .collect::<Vec<_>>();
    for row in &rows {
        let id = row
            .referenced_id
            .as_deref()
            .or(row.id.as_deref())
            .ok_or(bad("referencedId required"))?;
        let qty = row.quantity.unwrap_or(1);
        if qty > 10000 {
            return Err(bad("Quantity too large"));
        }
        if let Some(i) = items.iter_mut().find(|i| i.id == id) {
            i.quantity = i
                .quantity
                .checked_add(qty as u32)
                .ok_or(bad("Quantity overflow"))?;
        } else {
            items.push(Item {
                id: id.into(),
                quantity: qty as u32,
            });
        }
    }
    let changed = set_items(&a, &h, items, Some(c.revision)).await?;
    Ok(Json(cart_json(&a, &changed).await?))
}
pub(crate) async fn place_order(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        crate::order_checkout::checkout(
            &a,
            &h,
            header(&h, "idempotency-key").ok_or(bad("Idempotency-Key required"))?,
        )
        .await?,
    ))
}
