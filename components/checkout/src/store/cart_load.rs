//! The buyer's cart of a request: loaded by tenant and context token, then guarded by cashier, company and sales channel rules.
use crate::store::carts::CartRow;
use veyndo_core::shop_scope::{channel, tenant, token};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_shop::types::cart::StoredCart;

/// The guarded cart of the request; a cart from another sales channel is refused.
pub async fn load(a: &State, h: &RequestContext) -> Result<StoredCart> {
    let c = sqlx::query_as!(
        CartRow,
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2",
        tenant(h)?,
        token(h)?
    )
    .fetch_optional(&a.db)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Cart not found".into()))?
    .into_cart()?;
    veyndo_shop::domain::cart_admission::request(h, &c)?;
    veyndo_b2b::store::access::guard(a, h, &c).await?;
    if c.data.sales_channel != channel(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Cart belongs to another sales channel".into(),
        ));
    }
    veyndo_marketing::service::channels::channel(a, h, &c.data.sales_channel, &c.data.locale)
        .await?;
    Ok(c)
}

/// After a customer signs out, the open cart forgets the buyer and its company context under a new token.
pub async fn forget_customer(a: &State, t: &str, token: &str, email: &str) -> Result<()> {
    sqlx::query!("UPDATE carts SET data=jsonb_set(jsonb_set(jsonb_set(jsonb_set(jsonb_set(data,'{email}','null'),'{customer_id}','null'),'{group}','\"consumer\"'),'{company}','null'),'{b2b}','null'),token=$1,revision=revision+1 WHERE tenant=$2 AND token=$3 AND status='open' AND data->>'email'=$4", veyndo_core::uid(), t, token, email).execute(&a.db).await?;
    Ok(())
}

/// Stores the cart with its new revision when the open cart still has the expected revision; false when it changed meanwhile.
pub async fn replace_open(a: &State, c: &StoredCart, expected: i64) -> Result<bool> {
    let n = sqlx::query!("UPDATE carts SET data=$1,revision=$2 WHERE tenant=$3 AND token=$4 AND revision=$5 AND status='open'", serde_json::json!(c.data), c.revision, c.tenant, c.token, expected)
        .execute(&a.db)
        .await?
        .rows_affected();
    Ok(n == 1)
}
