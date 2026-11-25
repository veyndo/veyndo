//! Where a delivery or payment method is still referenced; order snapshots remain immutable. The documents belong to other components and are counted through their port.
use crate::ports::{MethodReference, MethodReferences};
use serde::Serialize;
use sqlx::PgConnection;
use veyndo_core::{Result, bad};

#[derive(Clone, Copy)]
pub enum Area {
    Shipping,
    Payments,
}

impl Area {
    pub fn parse(area: &str) -> Result<Self> {
        match area {
            "shipping" => Ok(Area::Shipping),
            "payments" => Ok(Area::Payments),
            _ => Err(bad("Unknown method area")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Area::Shipping => "shipping",
            Area::Payments => "payments",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub rules: i64,
    pub orders: i64,
    pub carts: i64,
    pub overrides: i64,
    pub b2b_units: i64,
    pub can_delete: bool,
}

/// Counts the references to a method; with a channel, only that channel's carts and orders.
pub async fn usage(
    references: &dyn MethodReferences,
    tx: &mut PgConnection,
    t: &str,
    area: Area,
    id: &str,
    channel: Option<&str>,
) -> Result<Usage> {
    let (selection, legacy, snapshot, allowed) = match area {
        Area::Shipping => (
            "shippingMethodId",
            "shipping_method_id",
            vec!["cart", "shippingMethod", "id"],
            "allowedShipping",
        ),
        Area::Payments => (
            "paymentMethodId",
            "payment_method_id",
            vec!["payment", "method", "id"],
            "allowedPayments",
        ),
    };
    let m = MethodReference {
        tenant: t,
        id,
        channel,
        area: area.name(),
        snapshot: &snapshot,
        selection,
        legacy,
        allowed,
    };
    let orders = references.orders(&mut *tx, &m).await?;
    let carts = references.carts(&mut *tx, &m).await?;
    let overrides = if channel.is_none() {
        references.overrides(&mut *tx, &m).await?
    } else {
        0
    };
    let rules = references.rules(&mut *tx, &m).await?;
    let b2b_units = references.units(tx, &m).await?;
    Ok(Usage {
        rules,
        orders,
        carts,
        overrides,
        b2b_units,
        can_delete: orders + carts + overrides + rules + b2b_units == 0,
    })
}
