//! What a checkout session reads under the checkout transaction: the channel it may sell on and the order it already placed.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::store::environments::live_parent;
use veyndo_core::{Error, Result, StatusCode};
use veyndo_marketing::store::channel_admission::share_open;
use veyndo_orders::store::placement::session_order;
use veyndo_shop::types::cart::StoredCart;

/// Sessions sell only on an active public channel of a live shop.
pub async fn admit_channel(tx: &mut PgConnection, c: &StoredCart) -> Result<()> {
    let open = share_open(&mut *tx, &c.tenant, &c.data.sales_channel).await?;
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
    session_order(tx, tenant, key, cart, channel)
        .await?
        .ok_or(Error(
            StatusCode::NOT_FOUND,
            "Checkout receipt unavailable".into(),
        ))
}
