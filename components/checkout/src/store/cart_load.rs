//! The buyer's cart of a request: loaded by tenant and context token, then guarded by cashier, company and sales channel rules.
use veyndo_core::shop_scope::{channel, tenant, token};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode};
use veyndo_shop::types::cart::{StoredCart, stored};

/// The guarded cart of the request; a cart from another sales channel is refused.
pub async fn load(a: &State, h: &RequestContext) -> Result<StoredCart> {
    let r = sqlx::query("SELECT * FROM carts WHERE tenant=$1 AND token=$2")
        .bind(tenant(h)?)
        .bind(token(h)?)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Cart not found".into()))?;
    let c = stored(&r)?;
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
