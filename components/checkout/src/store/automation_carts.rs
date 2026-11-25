//! The cart rows marketing reads and changes inside its own transactions: the coupon revision write, an order's cart for flow projection, the channel's cart count and the cart sign-out of a flow that changes a customer's authority. Marketing reaches them through its records port. Columns checked at compile time.
use crate::store::carts::CartRow;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Stores the cart document and advances its revision by one.
pub async fn save_next_revision(
    conn: &mut PgConnection,
    data: &Value,
    id: &str,
    tenant: &str,
) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1,revision=revision+1 WHERE tenant=$3 AND id=$2",
        data,
        id,
        tenant
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Cart `id` of any shop, unlocked; an order names its cart by id.
pub async fn by_id(conn: &mut PgConnection, id: &str) -> Result<Option<CartRow>> {
    Ok(sqlx::query_as!(
        CartRow,
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE id=$1",
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// How many carts belong to sales channel `channel`.
pub async fn channel_count(conn: &mut PgConnection, tenant: &str, channel: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM carts WHERE tenant=$1 AND sales_channel_id=$2"#,
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}

/// Signs every open cart of the customer out: a fresh token derived from `seed`, no customer, the consumer group and the next revision.
pub async fn sign_out(
    conn: &mut PgConnection,
    seed: &str,
    tenant: &str,
    customer: Option<&str>,
) -> Result<()> {
    sqlx::query!("UPDATE carts SET token=md5(id||$1),data=jsonb_set(jsonb_set(jsonb_set(jsonb_set(data,'{email}','null'),'{customer_id}','null'),'{group}','\"consumer\"'),'{company}','null'),revision=revision+1 WHERE tenant=$2 AND data->>'customer_id'=$3 AND status='open'", seed, tenant, customer).execute(conn).await?;
    Ok(())
}
