//! Cart writes of the app platform: a configuration change at its revision and the completed cart behind an imported order.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Stores the changed document and revision of a cart locked in this transaction.
pub async fn save(conn: &mut PgConnection, id: &str, data: &Value, revision: i64) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1,revision=$2 WHERE id=$3",
        data,
        revision,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Stores the completed cart that backs an imported order.
pub async fn insert_completed(
    conn: &mut PgConnection,
    id: &str,
    t: &str,
    token: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO carts(id,tenant,token,data,status) VALUES($1,$2,$3,$4,'completed')",
        id,
        t,
        token,
        data
    )
    .execute(conn)
    .await?;
    Ok(())
}
