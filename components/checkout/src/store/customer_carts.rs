//! The carts of a buyer account for the customers component: the sign-in binding and the detachment after access changes.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;
use veyndo_shop::types::cart::{CartRow, StoredCart};

/// The cart of a context token under an update lock.
pub async fn lock_by_token(conn: &mut PgConnection, t: &str, token: &str) -> Result<StoredCart> {
    sqlx::query_as!(
        CartRow,
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE",
        t,
        token
    )
    .fetch_one(conn)
    .await?
    .hydrate()
}

/// Stores the bound cart document and its new context token.
pub async fn bind(conn: &mut PgConnection, id: &str, data: &Value, token: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1,token=$2,revision=revision+1 WHERE id=$3",
        data,
        token,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Clears the account fields of the open carts of an email and rotates their tokens with `salt`.
pub async fn detach(conn: &mut PgConnection, t: &str, email: &str, salt: &str) -> Result<()> {
    sqlx::query!(r#"UPDATE carts SET token=md5(id||$1),data=jsonb_set(jsonb_set(jsonb_set(jsonb_set(data,'{email}','null'),'{customer_id}','null'),'{group}','"consumer"'),'{company}','null'),revision=revision+1 WHERE tenant=$2 AND data->>'email'=$3 AND status='open'"#, salt, t, email)
        .execute(conn)
        .await?;
    Ok(())
}
