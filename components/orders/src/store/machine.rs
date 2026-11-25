//! The shop's stored order state machine; a shop without one uses the default.
use crate::domain::order_machine::{OrderMachine, default_machine};
use veyndo_core::{Result, bad};
use veyndo_runtime::scoped_pool::ScopedPool;

pub async fn machine(db: &ScopedPool, t: &str) -> Result<(OrderMachine, i64)> {
    let mut conn = db.begin().await?;
    machine_conn(&mut conn, t).await
}
pub async fn machine_conn(conn: &mut sqlx::PgConnection, t: &str) -> Result<(OrderMachine, i64)> {
    let r = sqlx::query!(
        "SELECT data,revision FROM order_state_machines WHERE tenant=$1",
        t
    )
    .fetch_optional(conn)
    .await?;
    match r {
        Some(r) => Ok((
            serde_json::from_value(r.data).map_err(|_| bad("Invalid stored workflow"))?,
            r.revision,
        )),
        None => Ok((default_machine(), 0)),
    }
}
