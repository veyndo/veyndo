//! Revision-safe coupon selection for shared storefront/POS carts; pricing remains in promotions.
use crate::store::carts as cart_rows;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{tenant, token};
use veyndo_core::{RequestContext, Result, State, bad, conflict, store::history_context};
use veyndo_shop::ports::Carts;

/// The JSON fields `select` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SelectInput {
    codes: Value,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Replaces the coupon codes of the request's cart and returns its priced view.
pub async fn select(carts: &dyn Carts, a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: SelectInput = view(v);
    let codes = input
        .codes
        .as_array()
        .filter(|v| v.len() <= 5)
        .ok_or(bad("Maximum five coupon codes"))?;
    let codes = codes
        .iter()
        .map(|c| {
            c.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 64 && s.is_ascii())
                .map(str::to_string)
                .ok_or(bad("Invalid coupon code"))
        })
        .collect::<Result<Vec<_>>>()?;
    let t = tenant(h)?;
    let mut tx = a.db.begin().await?;
    history_context::context(&mut tx, h, "merchant").await?;
    let mut c = cart_rows::lock(&mut tx, &t, token(h)?).await?;
    carts.admit(&mut tx, h, &c).await?;
    if c.status != "open" || Some(c.revision) != input.revision {
        return Err(conflict("Cart changed or terminal"));
    }
    c.data.coupons = codes;
    cart_rows::save_next_revision(&mut tx, &json!(c.data), &c.id, &t).await?;
    c.revision += 1;
    tx.commit().await?;
    carts.view(&c).await
}
