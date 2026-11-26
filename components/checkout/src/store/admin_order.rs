//! Locked checkout guards of an order a merchant places: channel policy, the selected customer and the channel's product selection.
use crate::store::selected_customer;
use serde::Deserialize;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use veyndo_catalog::store::purchase_products::outside_selection;
use veyndo_core::document::view;
use veyndo_core::{Error, Result, StatusCode, bad};
use veyndo_customers::store::purchase_customers::lock_id;
use veyndo_marketing::store::channel_admission::{product_selection, share_document};
use veyndo_shop::store::settings::scoped_locked;
use veyndo_shop::types::cart::StoredCart;

/// The sales channel fields that admit a merchant order.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ChannelAdmission {
    active: Value,
    visibility: Value,
    kind: Value,
}

/// Freeze channel policy in the existing checkout transaction before acquiring customer/cart locks.
pub async fn lock_checkout(
    conn: &mut Transaction<'_, Postgres>,
    c: &StoredCart,
    conditions: &(dyn Fn(&Value) -> Result<()> + Sync),
) -> Result<()> {
    if c.data.admin_actor.is_none() || c.status != "open" {
        return Ok(());
    }
    scoped_locked(conn, &c.tenant, &c.data.sales_channel, conditions).await?;
    let data = share_document(conn, &c.tenant, &c.data.sales_channel)
        .await?
        .ok_or(bad("Sales channel unavailable"))?;
    let channel: ChannelAdmission = view(&data);
    if channel.active != true
        || channel.visibility == "private"
        || !matches!(channel.kind.as_str(), Some("storefront" | "headless"))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Sales channel is paused or private; open an authorized merchant preview".into(),
        ));
    }
    let customer = c
        .data
        .customer_id
        .as_deref()
        .ok_or(bad("Manual order customer required"))?;
    lock_id(conn, &c.tenant, customer)
        .await?
        .ok_or(bad("Customer unavailable"))?;
    selected_customer::check(conn, c).await?;
    Ok(())
}

/// Recheck the frozen channel product selection against the locked native product snapshot.
pub async fn admit_products(tx: &mut Transaction<'_, Postgres>, c: &StoredCart) -> Result<()> {
    if c.data.admin_actor.is_none() {
        return Ok(());
    }
    let selected = product_selection(tx, &c.tenant, &c.data.sales_channel).await?;
    let selected: Vec<String> =
        serde_json::from_value(selected).map_err(|_| bad("Invalid channel catalog selection"))?;
    if selected.is_empty() {
        return Ok(());
    }
    let ids = c
        .data
        .items
        .iter()
        .map(|i| i.id.clone())
        .collect::<Vec<_>>();
    let denied = outside_selection(tx, &c.tenant, &ids, &selected).await?;
    if denied {
        return Err(bad(
            "Product unavailable in sales channel; review the order again",
        ));
    }
    Ok(())
}
