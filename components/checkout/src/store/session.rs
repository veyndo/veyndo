//! What a checkout session reads under the checkout transaction: the channel it may sell on and the order it already placed.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::shop_scope::live_parent;
use veyndo_core::{Error, Result, StatusCode};
use veyndo_shop::types::cart::StoredCart;

/// Sessions sell only on an active public channel of a live shop.
pub async fn admit_channel(tx: &mut PgConnection, c: &StoredCart) -> Result<()> {
    let open: Option<bool> = sqlx::query_scalar(
        "SELECT (data->>'active')::boolean AND coalesce(data->>'visibility','public')='public' FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE",
    )
    .bind(&c.tenant).bind(&c.data.sales_channel).fetch_optional(&mut *tx).await?;
    if open != Some(true) || live_parent(tx, &c.tenant).await?.is_some() {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "A live public channel is required".into(),
        ));
    }
    Ok(())
}

/// The order a session placed for this cart on this channel.
pub async fn order(
    tx: &mut PgConnection,
    tenant: &str,
    key: &str,
    cart: &str,
    channel: &str,
) -> Result<Value> {
    sqlx::query_scalar(
        "SELECT data FROM orders WHERE tenant=$1 AND idempotency_key=$2 AND cart_id=$3 AND data->>'salesChannelId'=$4",
    )
    .bind(tenant).bind(key).bind(cart).bind(channel)
    .fetch_optional(tx).await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Checkout receipt unavailable".into()))
}
