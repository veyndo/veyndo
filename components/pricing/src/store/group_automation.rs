//! The automation data of a customer group as flows change it: created on first use, locked for the change and stored with the next revision. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// The automation data of group `id`, creating the group row when it is missing and locking it.
pub async fn lock(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Value> {
    sqlx::query!(
        "INSERT INTO commerce_customer_groups(tenant,id) VALUES($1,$2) ON CONFLICT DO NOTHING",
        tenant,
        id
    )
    .execute(&mut *conn)
    .await?;
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_customer_groups WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_one(conn)
    .await?)
}

/// Stores the automation data of group `id` and advances its revision.
pub async fn save(conn: &mut PgConnection, data: &Value, tenant: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE commerce_customer_groups SET data=$1,revision=revision+1 WHERE tenant=$2 AND id=$3",
        data,
        tenant,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}
