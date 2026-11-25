//! Order rows as rules and flows read and change them: a customer's order history, the locked order document and its flow activity journal, the revision-bound automation write, issued receipt kinds, an order's cart and the channel's order count. Marketing reaches them through its records port. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// A customer's order count, total amount and days since the last order.
pub struct CustomerOrders {
    pub count: i64,
    pub amount: f64,
    pub days: Option<f64>,
}

/// The order history of customer `customer`.
pub async fn customer_orders(
    conn: &mut PgConnection,
    tenant: &str,
    customer: Option<&str>,
) -> Result<CustomerOrders> {
    Ok(sqlx::query_as!(CustomerOrders, r#"SELECT count(*) AS "count!",coalesce(sum((data->'cart'->'price'->>'totalPrice')::numeric),0)::double precision AS "amount!",abs(current_date-max(created_at)::date)::double precision AS days FROM orders WHERE tenant=$1 AND data->'orderCustomer'->>'customerId'=$2"#, tenant, customer)
        .fetch_one(conn)
        .await?)
}

/// The activity a flow step with idempotency key `key` already journaled on order `id`.
pub async fn flow_activity(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    key: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM order_activity WHERE tenant=$1 AND order_id=$2 AND data->>'flowKey'=$3",
        tenant,
        id,
        key
    )
    .fetch_optional(conn)
    .await?)
}

/// The document of order `id`, locked for the change.
pub async fn lock_data(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_one(conn)
    .await?)
}

/// Stores the document of order `id`.
pub async fn save_data(
    conn: &mut PgConnection,
    data: &Value,
    tenant: &str,
    id: &str,
) -> Result<()> {
    sqlx::query!(
        "UPDATE orders SET data=$1 WHERE tenant=$2 AND id=$3",
        data,
        tenant,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Journals the effect of a flow step on order `id`.
pub async fn add_flow_activity(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
    actor: Option<&str>,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,'flow',$4)",
        tenant,
        id,
        actor,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Stores the automation data of order `id` only at revision `rev`, advancing the document revision; the number of rows changed.
pub async fn save_automation_at(
    a: &State,
    tenant: &str,
    id: &str,
    data: &Value,
    rev: i64,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE orders SET data=jsonb_set(jsonb_set(data,'{automation}',$1),'{revision}',to_jsonb($4::bigint+1)) WHERE tenant=$2 AND id=$3 AND (data->>'revision')::bigint=$4", data, tenant, id, rev).execute(&a.db).await?.rows_affected())
}

/// How many orders belong to sales channel `channel`.
pub async fn channel_count(conn: &mut PgConnection, tenant: &str, channel: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM orders WHERE tenant=$1 AND data->>'salesChannelId'=$2"#,
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}

/// The kinds of the receipts issued for order `id`.
pub async fn receipt_kinds(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT kind FROM order_receipts WHERE tenant=$1 AND order_id=$2",
        tenant,
        id
    )
    .fetch_all(conn)
    .await?)
}

/// An order's document and the id of the cart it was placed from.
pub struct OrderCart {
    pub cart_id: Option<String>,
    pub data: Value,
}

/// Order `id` with its cart id; none for an unknown order.
pub async fn with_cart(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<OrderCart>> {
    Ok(sqlx::query_as!(
        OrderCart,
        "SELECT cart_id,data FROM orders WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}
