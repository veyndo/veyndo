//! Receipt settings and the order workflow of a sandbox shop: copying and locked snapshot reads, inside the caller's transaction.
use serde_json::Value;
use veyndo_core::Result;

/// The order workflow document, locked.
pub async fn order_workflow(c: &mut sqlx::PgConnection, t: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM order_state_machines WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_optional(&mut *c)
    .await?)
}

/// Whether an active order is in a state outside the given workflow states.
pub async fn stranded_orders(
    c: &mut sqlx::PgConnection,
    t: &str,
    states: Vec<String>,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM orders WHERE tenant=$1 AND NOT(data->>'state'=ANY($2)) AND data->>'state' NOT IN ('expired','payment_review')) AS "exists!""#, t, &states).fetch_one(&mut *c).await?)
}

/// Copies receipt settings and the order workflow into the sandbox.
pub async fn copy_receipts_and_workflow(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO receipt_settings(tenant,data) SELECT $2,data FROM receipt_settings WHERE tenant=$1",
        t,
        id
    )
    .execute(&mut *c)
    .await?;
    sqlx::query!("INSERT INTO order_state_machines(tenant,data) SELECT $2,data FROM order_state_machines WHERE tenant=$1", t, id).execute(&mut *c).await?;
    Ok(())
}
