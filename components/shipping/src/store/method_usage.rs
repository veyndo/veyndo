//! Where a delivery or payment method is still referenced; order snapshots remain immutable.
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
    let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM orders o JOIN carts c ON c.id=o.cart_id WHERE o.tenant=$1 AND ($4::text IS NULL OR COALESCE(c.data->>'sales_channel','default')=$4) AND (o.data #>> $2::text[]=$3 OR o.data->'cart'->'checkout'->>$5=$3)")
        .bind(t)
        .bind(snapshot)
        .bind(id)
        .bind(channel)
        .bind(selection)
        .fetch_one(&mut *tx)
        .await?;
    let carts: i64 = sqlx::query_scalar("SELECT count(*) FROM carts WHERE tenant=$1 AND status='open' AND ($4::text IS NULL OR COALESCE(data->>'sales_channel','default')=$4) AND (data->'checkout'->>$2=$3 OR data->'checkout'->>$5=$3)")
        .bind(t)
        .bind(selection)
        .bind(id)
        .bind(channel)
        .bind(legacy)
        .fetch_one(&mut *tx)
        .await?;
    let overrides: i64 = if channel.is_none() {
        sqlx::query_scalar("SELECT count(*) FROM commerce_overrides WHERE tenant=$1 AND EXISTS(SELECT 1 FROM jsonb_array_elements(data) p WHERE p->'path'->>0=$2 AND p->'path'->>1=$3)")
            .bind(t)
            .bind(area.name())
            .bind(id)
            .fetch_one(&mut *tx)
            .await?
    } else {
        0
    };
    let rules: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM commerce_rules WHERE tenant=$1 AND jsonb_path_exists(condition,'$.** ? (@ == $id)',jsonb_build_object('id',$2::text))) + (SELECT count(*) FROM commerce_flows WHERE tenant=$1 AND jsonb_path_exists(data,'$.** ? (@ == $id)',jsonb_build_object('id',$2::text)))")
        .bind(t)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let b2b_units: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM b2b_units WHERE tenant=$1 AND COALESCE(data->$2,'[]'::jsonb) ? $3",
    )
    .bind(t)
    .bind(allowed)
    .bind(id)
    .fetch_one(tx)
    .await?;
    Ok(Usage {
        rules,
        orders,
        carts,
        overrides,
        b2b_units,
        can_delete: orders + carts + overrides + rules + b2b_units == 0,
    })
}
